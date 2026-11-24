//! Type-2 (`BLAKE3`) and Type-3 (5-gram AST `MinHash`) clone fingerprinting (`GF020`)
//! plus homonymy-safe symbol reference extraction (`GF021`).

mod symbol_refs;

use tree_sitter::Node;

use crate::types::node_text;
pub use symbol_refs::collect_referenced_symbols;

const COMMENT_KINDS: &[&str] = &["comment", "line_comment", "block_comment"];

const IDENT_KINDS: &[&str] = &[
    "identifier",
    "field_identifier",
    "type_identifier",
    "property_identifier",
    "shorthand_property_identifier",
    "shorthand_property_identifier_pattern",
];

const LITERAL_KINDS: &[&str] = &[
    "integer_literal",
    "float_literal",
    "string_literal",
    "raw_string_literal",
    "char_literal",
    "boolean_literal",
    "number",
    "string",
    "integer",
    "float",
    "true",
    "false",
    "none",
    "null",
];

const NOISE_PUNCTUATION: &[&str] = &["{", "}", "(", ")", "[", "]", ";", ",", ":", "."];

const MIN_SHINGLE_AST_NODES: u32 = 20;
const SHINGLE_WINDOW_SIZE: usize = 5;

/// Normalized Type-2 and Type-3 AST fingerprint of a callable body.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyFingerprint {
    /// Exact `BLAKE3` 128-bit digest of depth-tagged normalized AST tokens.
    pub structural_hash: Option<u128>,
    /// Deterministic 8-register `MinHash` over 5-gram AST shingles (`ast_node_count >= 20`).
    pub shingle_sketch: Option<[u64; 8]>,
    /// Total named non-comment AST nodes inside the body.
    pub ast_node_count: u32,
}

struct BodyFingerprinter<'a> {
    source: &'a str,
    exact_hasher: blake3::Hasher,
    shingle_tokens: Vec<&'a str>,
    ast_node_count: u32,
}

impl<'a> BodyFingerprinter<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            exact_hasher: blake3::Hasher::new(),
            shingle_tokens: Vec::new(),
            ast_node_count: 0,
        }
    }

    fn feed_exact(&mut self, depth: u16, token: &str) {
        self.exact_hasher.update(&depth.to_le_bytes());
        self.exact_hasher.update(token.as_bytes());
        self.exact_hasher.update(&[0xFF]);
    }

    fn visit(&mut self, node: Node<'a>, depth: u16) {
        let kind = node.kind();
        if COMMENT_KINDS.contains(&kind) {
            return;
        }
        if !node.is_named() {
            let tok = node_text(node, self.source).trim();
            if !tok.is_empty() {
                self.feed_exact(depth, tok);
                if !NOISE_PUNCTUATION.contains(&tok) {
                    self.shingle_tokens.push(tok);
                }
            }
            return;
        }

        self.ast_node_count = self.ast_node_count.saturating_add(1);
        if IDENT_KINDS.contains(&kind) {
            self.feed_exact(depth, "$ID");
            self.shingle_tokens.push("$ID");
            return;
        }
        if LITERAL_KINDS.contains(&kind) {
            self.feed_exact(depth, "$LIT");
            self.shingle_tokens.push("$LIT");
            return;
        }

        self.feed_exact(depth, kind);
        self.shingle_tokens.push(kind);
        let next_depth = depth.saturating_add(1);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit(child, next_depth);
        }
    }

    fn finalize(self) -> BodyFingerprint {
        if self.ast_node_count == 0 {
            return BodyFingerprint {
                structural_hash: None,
                shingle_sketch: None,
                ast_node_count: 0,
            };
        }
        let digest = self.exact_hasher.finalize();
        let mut first_16 = [0_u8; 16];
        first_16.copy_from_slice(&digest.as_bytes()[..16]);
        let shingle_sketch = (self.ast_node_count >= MIN_SHINGLE_AST_NODES)
            .then(|| compute_shingle_sketch(&self.shingle_tokens))
            .flatten();
        BodyFingerprint {
            structural_hash: Some(u128::from_le_bytes(first_16)),
            shingle_sketch,
            ast_node_count: self.ast_node_count,
        }
    }
}

fn hash_shingle_window(window: &[&str]) -> [u64; 8] {
    let mut hasher = blake3::Hasher::new();
    for tok in window {
        hasher.update(tok.as_bytes());
        hasher.update(&[0xFF]);
    }
    let mut xof_bytes = [0_u8; 64];
    hasher.finalize_xof().fill(&mut xof_bytes);
    let mut words = [0_u64; 8];
    let (chunks, _) = xof_bytes.as_chunks::<8>();
    for (slot, chunk) in words.iter_mut().zip(chunks) {
        *slot = u64::from_le_bytes(*chunk);
    }
    words
}

fn compute_shingle_sketch(tokens: &[&str]) -> Option<[u64; 8]> {
    if tokens.len() < SHINGLE_WINDOW_SIZE {
        return None;
    }
    let mut min_hashes = [u64::MAX; 8];
    for window in tokens.windows(SHINGLE_WINDOW_SIZE) {
        let hashes = hash_shingle_window(window);
        for (min_slot, h) in min_hashes.iter_mut().zip(hashes) {
            *min_slot = (*min_slot).min(h);
        }
    }
    Some(min_hashes)
}

/// Traverses `body` once to compute its Type-2 `BLAKE3` hash, Type-3 `MinHash` sketch, and node count.
pub fn fingerprint_body(body: Node<'_>, source: &str) -> BodyFingerprint {
    let mut fp = BodyFingerprinter::new(source);
    fp.visit(body, 0);
    fp.finalize()
}
