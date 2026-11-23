//! Normalized structural and complexity metadata extracted from `Tree-sitter` CSTs.

use alloc::string::String;
use alloc::vec::Vec;

use crate::diagnostic::Span;

/// Concrete syntax error or missing token reported by `Tree-sitter`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxErrorNode {
    /// Short description of the malformed or missing syntax construct.
    pub message: String,
    /// Source span of the `ERROR` or `MISSING` node.
    pub span: Span,
}

/// Normalized module import declaration (`use`, `import`, `from ... import`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDecl {
    /// Imported module path or specifier.
    pub module_source: String,
    /// Explicit imported symbol names (empty for wildcard/namespace imports).
    pub symbols: Vec<String>,
    /// Source span of the import statement.
    pub span: Span,
}

/// Normalized public symbol export declaration (`pub`, `export`, Python public symbol).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportDecl {
    /// Exported symbol identifier.
    pub symbol: String,
    /// Source span of the exported declaration.
    pub span: Span,
}

/// Kind of composite type or implementation block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    /// Rust `struct`.
    Struct,
    /// Rust or TypeScript `enum`.
    Enum,
    /// Python or TypeScript/JavaScript `class`.
    Class,
    /// TypeScript `interface` or Rust `trait`.
    Interface,
    /// Rust `impl` block.
    Impl,
}

/// Normalized composite type or `impl` declaration with SRP metrics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDecl {
    /// Identifier of the type or `impl` target.
    pub name: String,
    /// Kind of type construct.
    pub kind: TypeKind,
    /// Source span of the declaration.
    pub span: Span,
    /// Number of declared fields, properties, or variants.
    pub field_count: u32,
    /// Number of associated methods or member functions.
    pub method_count: u32,
}

/// Kind of executable callable unit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableKind {
    /// Standalone function.
    Function,
    /// Associated method on a class, trait, or `impl` block.
    Method,
    /// Named closure or assigned arrow function.
    Closure,
}

/// Normalized function, method, or named closure with complexity metrics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDecl {
    /// Identifier of the callable.
    pub name: String,
    /// Kind of callable construct.
    pub kind: CallableKind,
    /// Source span of the callable definition.
    pub span: Span,
    /// Non-blank lines inside the callable span.
    pub effective_lines: u32,
    /// Declared parameter count (excluding receiver `self` / `cls`).
    pub param_count: u32,
    /// `McCabe` cyclomatic complexity (`>= 1`).
    pub cyclomatic_complexity: u32,
    /// Cognitive complexity including control-flow nesting penalties.
    pub cognitive_complexity: u32,
    /// Deepest control-flow nesting level reached inside the callable body.
    pub max_nesting_depth: u32,
}
