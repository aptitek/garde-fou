//! W3C CSS Syntax Level 3 structural parser over `std::slice::Iter<CssToken>` (zero index arithmetic).

use std::slice::Iter;

use super::css_ast_nodes::strip_trailing_important;
pub use super::css_ast_nodes::{CssDeclaration, CssRule, CssValue};
use super::tokenizer::{CssTokKind, CssToken};

struct Cursor<'a> {
    iter: Iter<'a, CssToken>,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<&'a CssToken> {
        self.iter.as_slice().first()
    }

    fn next(&mut self) -> Option<&'a CssToken> {
        self.iter.next()
    }

    fn parse_component_value(&mut self) -> Option<CssValue> {
        let tok = self.next()?;
        if let CssTokKind::Function(name) = &tok.kind {
            let mut args = Vec::new();
            while let Some(p) = self.peek() {
                if p.kind == CssTokKind::Close(')') {
                    self.next();
                    break;
                }
                args.extend(self.parse_component_value());
            }
            return Some(CssValue::Function {
                name: name.clone(),
                line: tok.line,
                args,
            });
        }
        Some(CssValue::Token(tok.clone()))
    }

    fn collect_prelude_until_block_or_semi(&mut self) -> (Vec<CssValue>, Option<char>) {
        let mut prelude = Vec::new();
        while let Some(p) = self.peek() {
            match p.kind {
                CssTokKind::Semicolon => {
                    self.next();
                    return (prelude, Some(';'));
                }
                CssTokKind::Open('{') => {
                    self.next();
                    return (prelude, Some('{'));
                }
                CssTokKind::Close('}') => return (prelude, None),
                _ => prelude.extend(self.parse_component_value()),
            }
        }
        (prelude, None)
    }

    fn looks_like_declaration(&self) -> bool {
        let slice = self.iter.as_slice();
        let [
            CssToken {
                kind: CssTokKind::Ident(_),
                ..
            },
            CssToken {
                kind: CssTokKind::Colon,
                ..
            },
            rest @ ..,
        ] = slice
        else {
            return false;
        };
        let mut depth = 0usize;
        for t in rest {
            match &t.kind {
                CssTokKind::Function(_) | CssTokKind::Open('(' | '[') => {
                    depth = depth.saturating_add(1);
                }
                CssTokKind::Close(')' | ']') => depth = depth.saturating_sub(1),
                CssTokKind::Open('{') if depth == 0 => return false,
                CssTokKind::Semicolon | CssTokKind::Close('}') if depth == 0 => return true,
                _ => {}
            }
        }
        true
    }

    fn parse_declaration(&mut self) -> Option<CssDeclaration> {
        let prop_tok = self.next()?;
        let CssTokKind::Ident(property) = &prop_tok.kind else {
            return None;
        };
        if self.peek().map(|t| &t.kind) == Some(&CssTokKind::Colon) {
            self.next();
        }
        let mut raw_vals = Vec::new();
        while let Some(p) = self.peek() {
            match p.kind {
                CssTokKind::Semicolon => {
                    self.next();
                    break;
                }
                CssTokKind::Close('}') => break,
                _ => raw_vals.extend(self.parse_component_value()),
            }
        }
        let important = strip_trailing_important(&mut raw_vals);
        Some(CssDeclaration {
            property: property.clone(),
            line: prop_tok.line,
            values: raw_vals,
            important,
        })
    }

    fn step_block_item(
        &mut self,
        declarations: &mut Vec<CssDeclaration>,
        nested_rules: &mut Vec<CssRule>,
    ) -> bool {
        let Some(p) = self.peek() else {
            return false;
        };
        match &p.kind {
            CssTokKind::Close('}') => {
                self.next();
                return false;
            }
            CssTokKind::AtKeyword(_) => nested_rules.extend(self.parse_at_rule()),
            _ if self.looks_like_declaration() => declarations.extend(self.parse_declaration()),
            _ => nested_rules.extend(self.parse_qualified_rule()),
        }
        true
    }

    fn parse_block_contents(&mut self) -> (Vec<CssDeclaration>, Vec<CssRule>) {
        let mut declarations = Vec::new();
        let mut nested_rules = Vec::new();
        while self.step_block_item(&mut declarations, &mut nested_rules) {}
        (declarations, nested_rules)
    }

    fn parse_at_rule(&mut self) -> Option<CssRule> {
        let tok = self.next()?;
        let CssTokKind::AtKeyword(raw_name) = &tok.kind else {
            return None;
        };
        let line = tok.line;
        let name = raw_name.to_ascii_lowercase();
        let (prelude, sep) = self.collect_prelude_until_block_or_semi();
        let (declarations, nested_rules) = if sep == Some('{') {
            self.parse_block_contents()
        } else {
            (Vec::new(), Vec::new())
        };
        Some(CssRule::AtRule {
            name,
            line,
            prelude,
            declarations,
            nested_rules,
        })
    }

    fn parse_qualified_rule(&mut self) -> Option<CssRule> {
        let (prelude, sep) = self.collect_prelude_until_block_or_semi();
        if sep != Some('{') {
            return (!prelude.is_empty()).then_some(CssRule::QualifiedRule {
                prelude,
                declarations: Vec::new(),
                nested_rules: Vec::new(),
            });
        }
        let (declarations, nested_rules) = self.parse_block_contents();
        Some(CssRule::QualifiedRule {
            prelude,
            declarations,
            nested_rules,
        })
    }
}

/// Parses a CSS token stream into a list of top-level [`CssRule`] nodes.
#[must_use]
pub fn parse_css_stylesheet(tokens: &[CssToken]) -> Vec<CssRule> {
    let mut cur = Cursor {
        iter: tokens.iter(),
    };
    let mut rules = Vec::new();
    while let Some(p) = cur.peek() {
        match p.kind {
            CssTokKind::Close('}') => {
                cur.next();
            }
            CssTokKind::AtKeyword(_) => rules.extend(cur.parse_at_rule()),
            _ => rules.extend(cur.parse_qualified_rule()),
        }
    }
    rules
}

/// Parses inline `style="..."` attribute declarations.
#[must_use]
pub fn parse_inline_css_declarations(tokens: &[CssToken]) -> Vec<CssDeclaration> {
    let mut cur = Cursor {
        iter: tokens.iter(),
    };
    let mut decls = Vec::new();
    while let Some(p) = cur.peek() {
        if p.kind == CssTokKind::Semicolon {
            cur.next();
        } else if cur.looks_like_declaration() {
            decls.extend(cur.parse_declaration());
        } else {
            cur.next();
        }
    }
    decls
}
