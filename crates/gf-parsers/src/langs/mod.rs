//! Language-specific `Tree-sitter` and pure-Rust multi-domain extractors.

pub mod asset;
pub mod css;
pub mod dockerfile;
mod dockerfile_instr;
pub mod i18n_dict;
pub mod jsonc;
pub mod markdown;
mod markdown_scan;
pub mod python;
mod python_calls;
mod python_exports;
mod python_guardrails;
pub(crate) mod python_typing;
pub mod rust;
mod rust_effects;
mod rust_guardrails;
pub mod tool_config;
mod tool_config_py;
pub mod typescript;
mod typescript_jsx;
mod typescript_ui;
