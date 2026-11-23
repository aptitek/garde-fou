//! Idiomatic strict Rust Edition 2024 scaffolding templates (`Cargo.toml`, `src/lib.rs`).

fn render_cargo_toml(project_kebab: &str) -> String {
    format!(
        "[package]\n\
         name = \"{project_kebab}\"\n\
         version = \"0.1.0\"\n\
         edition = \"2024\"\n\
         \n\
         [lints.rust]\n\
         unsafe_code = \"forbid\"\n\
         missing_docs = \"deny\"\n\
         \n\
         [lints.clippy]\n\
         all = {{ level = \"deny\", priority = -1 }}\n\
         pedantic = {{ level = \"deny\", priority = -1 }}\n\
         unwrap_used = \"deny\"\n\
         expect_used = \"deny\"\n\
         panic = \"deny\"\n\
         todo = \"deny\"\n\
         dbg_macro = \"deny\"\n"
    )
}

fn render_lib_rs(pkg_snake: &str) -> String {
    format!(
        "//! Core domain library for `{pkg_snake}`.\n\
         \n\
         /// Computes a deterministic greeting banner for `subject`.\n\
         #[must_use]\n\
         pub fn build_banner(subject: &str) -> String {{\n\
         \x20   let trimmed = subject.trim();\n\
         \x20   if trimmed.is_empty() {{\n\
         \x20       String::from(\"hello, world\")\n\
         \x20   }} else {{\n\
         \x20       format!(\"hello, {{trimmed}}\")\n\
         \x20   }}\n\
         }}\n\
         \n\
         #[cfg(test)]\n\
         mod tests {{\n\
         \x20   use super::build_banner;\n\
         \n\
         \x20   #[test]\n\
         \x20   fn formats_non_empty_and_blank_subjects() -> Result<(), &'static str> {{\n\
         \x20       if build_banner(\"garde-fou\") != \"hello, garde-fou\" {{\n\
         \x20           return Err(\"unexpected greeting for named subject\");\n\
         \x20       }}\n\
         \x20       if build_banner(\"   \") != \"hello, world\" {{\n\
         \x20           return Err(\"unexpected fallback greeting\");\n\
         \x20       }}\n\
         \x20       Ok(())\n\
         \x20   }}\n\
         }}\n"
    )
}

/// Generates non-destructive Rust starter files `(relative_path, contents)`.
pub fn rust_files(project_kebab: &str, pkg_snake: &str) -> Vec<(String, String)> {
    vec![
        (String::from("Cargo.toml"), render_cargo_toml(project_kebab)),
        (String::from("src/lib.rs"), render_lib_rs(pkg_snake)),
    ]
}
