//! Normalized structural, complexity, AST fingerprint, and effect-sighting metadata.

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
    /// `true` when every imported binding is erased at compile time (`import type` / `{ type X }`).
    #[serde(default)]
    pub is_type_only: bool,
    /// Source span of the import statement.
    pub span: Span,
}

/// Normalized symbol export declaration (`pub`, `pub(crate)`, `export`, Python public symbol).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExportDecl {
    /// Exported symbol identifier.
    pub symbol: String,
    /// `true` for `pub(crate)` / `pub(super)` items tracked for crate-internal dead code (`GF021`).
    pub is_crate_internal: bool,
    /// `true` when exporting exclusively in the TypeScript type namespace (`export type` / `interface`).
    #[serde(default)]
    pub is_type_only: bool,
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

/// Normalized composite type or `impl` declaration with SRP and boolean-field metrics.
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
    /// Number of direct `bool` / `boolean` fields inspected by `GF009`.
    pub bool_field_count: u32,
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

/// Normalized function, method, or named closure with complexity and Type-2/Type-3 clone metrics.
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
    /// Deterministic 8-register `MinHash` sketch over 5-gram AST shingles for Type-3 clones (`GF020`).
    pub shingle_sketch: Option<[u64; 8]>,
    /// Total number of non-comment AST nodes inside the callable body.
    pub ast_node_count: u32,
}

/// Category of architectural effect or error-swallowing construct (`GF011` & `GF078`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum EffectSightingKind {
    /// Presence of `#![no_std]` crate-level attribute (`GF078`).
    HasNoStdAttr,
    /// Usage or import of `std::` outside `#[cfg(test)]` (`GF078`).
    StdUsage,
    /// Import or call to I/O primitives (`fs`, `net`, `process`, `env`, `GF078`).
    IoImportOrCall,
    /// Console output primitive (`println!`, `eprintln!`, `console.log`, `print`, `GF078`).
    ConsoleOutput,
    /// Silent error absorption (`let _ = fallible()`, `.unwrap_or_default()`, `GF011`).
    SilentErrorSwallow,
}

/// Architectural effect or silent error-swallowing sighting attached to [`crate::UastDocument`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EffectSighting {
    /// Category of effect or error-swallowing sighting.
    pub kind: EffectSightingKind,
    /// Matched symbol, path, or statement summary.
    pub detail: String,
    /// 1-indexed source span.
    pub span: Span,
}
