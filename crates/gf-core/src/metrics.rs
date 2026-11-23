//! Normalized structural, complexity, and AST fingerprint metadata.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::diagnostic::Span;

/// Concrete syntax error or missing token reported by a parser.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SyntaxErrorNode {
    /// Short description of the malformed or missing syntax construct.
    pub message: String,
    /// Source span of the `ERROR` or `MISSING` node.
    pub span: Span,
}

/// Redacted hardcoded secret or private key detected natively in source text (`GF040`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SecretFinding {
    /// Category slug (e.g. `"github-pat"`, `"pem-private-key"`, `"hardcoded-secret"`).
    pub kind: String,
    /// Masked representation safe for logs and reports (never contains the raw secret).
    pub redacted_preview: String,
    /// 1-indexed line span of the finding.
    pub span: Span,
}

/// Normalized module import declaration (`use`, `import`, `from ... import`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportDecl {
    /// Imported module path or specifier.
    pub module_source: String,
    /// Explicit imported symbol names (empty for wildcard/namespace imports).
    pub symbols: Vec<String>,
    /// Source span of the import statement.
    pub span: Span,
}

/// Normalized public symbol export declaration (`pub`, `export`, Python public symbol).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExportDecl {
    /// Exported symbol identifier.
    pub symbol: String,
    /// Source span of the exported declaration.
    pub span: Span,
}

/// Kind of composite type or implementation block.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CallableKind {
    /// Standalone function.
    Function,
    /// Associated method on a class, trait, or `impl` block.
    Method,
    /// Named closure or assigned arrow function.
    Closure,
}

/// Normalized function, method, or named closure with complexity and clone-detection metrics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
    /// Type-2 normalized `BLAKE3` 128-bit structural AST hash (`$ID` / `$LIT` normalized).
    pub structural_hash: Option<u128>,
    /// Total number of non-comment AST nodes inside the callable body.
    pub ast_node_count: u32,
}
