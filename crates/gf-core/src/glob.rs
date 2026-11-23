//! Pure `#![no_std]` path glob matcher supporting `*`, `**`, and `?` over `/` segments.

use alloc::vec::Vec;

const fn segment_matches(pat: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t) = (0_usize, 0_usize);
    let (mut star_p, mut star_t) = (None, 0_usize);

    while t < text.len() {
        if p < pat.len() && (pat[p] == b'?' || pat[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pat.len() && pat[p] == b'*' {
            star_p = Some(p);
            p += 1;
            star_t = t;
        } else if let Some(sp) = star_p {
            p = sp + 1;
            star_t += 1;
            t = star_t;
        } else {
            return false;
        }
    }

    while p < pat.len() && pat[p] == b'*' {
        p += 1;
    }
    p == pat.len()
}

fn match_segments(pats: &[&str], segs: &[&str]) -> bool {
    if pats.is_empty() {
        return segs.is_empty();
    }
    if pats[0] == "**" {
        if match_segments(&pats[1..], segs) {
            return true;
        }
        return !segs.is_empty() && match_segments(pats, &segs[1..]);
    }
    if segs.is_empty() {
        return false;
    }
    segment_matches(pats[0].as_bytes(), segs[0].as_bytes())
        && match_segments(&pats[1..], &segs[1..])
}

/// Matches a normalized forward-slash repository path against a glob pattern (`*`, `**`, `?`).
#[must_use]
pub fn glob_matches(pattern: &str, path: &str) -> bool {
    let trimmed_pat = pattern.trim_matches('/');
    let trimmed_path = path.trim_matches('/');
    if trimmed_pat.is_empty() {
        return trimmed_path.is_empty();
    }
    let pattern_parts: Vec<&str> = trimmed_pat.split('/').collect();
    let path_parts: Vec<&str> = if trimmed_path.is_empty() {
        Vec::new()
    } else {
        trimmed_path.split('/').collect()
    };
    match_segments(&pattern_parts, &path_parts)
}

#[cfg(test)]
mod tests {
    use super::glob_matches;

    #[test]
    fn matches_single_and_double_star_path_globs() {
        assert!(glob_matches(
            "crates/*/src/lib.rs",
            "crates/gf-core/src/lib.rs"
        ));
        assert!(!glob_matches(
            "crates/*/src/lib.rs",
            "crates/gf-core/src/sub/lib.rs"
        ));
        assert!(glob_matches("src/pages/**", "src/pages/index.tsx"));
        assert!(glob_matches("src/pages/**", "src/pages/blog/slug.astro"));
        assert!(glob_matches(
            "**/tests/**",
            "crates/gf-rules/tests/rules_proptest.rs"
        ));
        assert!(glob_matches("**/tests/**", "tests/architecture.rs"));
        assert!(glob_matches("src/main.*", "src/main.ts"));
        assert!(!glob_matches("src/main.*", "src/helper.ts"));
    }
}
