//! Embedded bilingual FR/EN UI catalog (`locales/fr.toml` & `locales/en.toml`, `#![no_std]`).

use alloc::string::String;
use serde::{Deserialize, Serialize};

/// Active human-facing UI locale for `garde` CLI and TUI rendering.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum UiLocale {
    /// English (deterministic default in neutral/CI environments).
    #[default]
    En,
    /// French (`fr`).
    Fr,
}

impl UiLocale {
    /// Canonical 2-letter lowercase ISO code (`"en"` or `"fr"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Fr => "fr",
        }
    }
}

/// Configured `[ui] locale` policy (`auto` | `fr` | `en`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiLocalePolicy {
    /// Detect from environment (`LC_ALL`/`LC_MESSAGES`/`LANG`), defaulting to `En`.
    #[default]
    Auto,
    /// Force French UI output.
    Fr,
    /// Force English UI output.
    En,
}

impl UiLocalePolicy {
    /// Parses `"auto"`, `"fr"`, or `"en"` (case-insensitive).
    #[must_use]
    pub fn from_config_str(raw: &str) -> Option<Self> {
        let low = raw.trim().to_ascii_lowercase();
        match low.as_str() {
            "auto" => Some(Self::Auto),
            "fr" | "fr_fr" | "fr-fr" => Some(Self::Fr),
            "en" | "en_us" | "en-us" | "en_gb" => Some(Self::En),
            _ => None,
        }
    }

    /// Canonical lowercase identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Fr => "fr",
            Self::En => "en",
        }
    }

    /// Resolves a concrete [`UiLocale`] given an optional host locale hint (e.g. `LANG`).
    #[must_use]
    pub fn resolve(self, env_locale: Option<&str>) -> UiLocale {
        match self {
            Self::Fr => UiLocale::Fr,
            Self::En => UiLocale::En,
            Self::Auto => {
                if env_locale.is_some_and(|v| v.trim().to_ascii_lowercase().starts_with("fr")) {
                    UiLocale::Fr
                } else {
                    UiLocale::En
                }
            }
        }
    }
}

const FR_TOML: &str = include_str!("../../../locales/fr.toml");
const EN_TOML: &str = include_str!("../../../locales/en.toml");

fn lookup_toml_key(raw: &'static str, dotted_key: &str) -> Option<&'static str> {
    let (section, key) = dotted_key.split_once('.')?;
    let mut in_section = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(sec) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            in_section = sec.trim() == section;
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=')
            && k.trim() == key
        {
            return Some(v.trim().trim_matches('"'));
        }
    }
    None
}

/// Looks up a bilingual UI message key in the embedded `locales/{fr,en}.toml` catalog.
#[must_use]
pub fn ui_tr(locale: UiLocale, key: &'static str) -> &'static str {
    let primary = match locale {
        UiLocale::Fr => FR_TOML,
        UiLocale::En => EN_TOML,
    };
    lookup_toml_key(primary, key)
        .or_else(|| lookup_toml_key(EN_TOML, key))
        .unwrap_or(key)
}

/// Formats a translated template by replacing `{name}` placeholders deterministically.
#[must_use]
pub fn ui_tr_fmt(locale: UiLocale, key: &'static str, vars: &[(&str, &str)]) -> String {
    let mut out = String::from(ui_tr(locale, key));
    for &(name, val) in vars {
        let pat = alloc::format!("{{{name}}}");
        out = out.replace(&pat, val);
    }
    out
}
