//! `serde` schemas for `cargo-mutants` (`outcomes.json`) and `Stryker` / `Mutmut`
//! (`mutation-testing-elements` `mutation.json`) reports (`GF052`).

use std::collections::BTreeMap;

use gf_core::Span;
use serde::Deserialize;

#[derive(Deserialize)]
struct RawLineCol {
    line: Option<u32>,
}

#[derive(Deserialize)]
struct RawMutantSpan {
    start: Option<RawLineCol>,
    end: Option<RawLineCol>,
}

impl RawMutantSpan {
    fn to_span(&self) -> Span {
        let start = self.start.as_ref().and_then(|p| p.line).unwrap_or(1).max(1);
        let end = self
            .end
            .as_ref()
            .and_then(|p| p.line)
            .unwrap_or(start)
            .max(start);
        Span::new(start, end).unwrap_or(Span::FIRST)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CargoMutantFn {
    Name(String),
    Obj { function_name: Option<String> },
}

impl CargoMutantFn {
    fn into_name(self) -> Option<String> {
        match self {
            Self::Name(s) => (!s.trim().is_empty()).then_some(s),
            Self::Obj { function_name } => function_name.filter(|s| !s.trim().is_empty()),
        }
    }
}

pub struct NormalizedRawSurvivor {
    pub raw_uri: Option<String>,
    pub fallback_path: &'static str,
    pub span: Span,
    pub function_name: Option<String>,
    pub mutator: String,
    pub replacement: Option<String>,
}

#[derive(Deserialize)]
pub struct CargoMutantDetail {
    file: Option<String>,
    function: Option<CargoMutantFn>,
    span: Option<RawMutantSpan>,
    replacement: Option<String>,
    genre: Option<String>,
}

impl CargoMutantDetail {
    pub fn into_normalized(self) -> NormalizedRawSurvivor {
        NormalizedRawSurvivor {
            raw_uri: self.file,
            fallback_path: "Cargo.toml",
            span: self
                .span
                .as_ref()
                .map_or(Span::FIRST, RawMutantSpan::to_span),
            function_name: self.function.and_then(CargoMutantFn::into_name),
            mutator: self
                .genre
                .filter(|g| !g.trim().is_empty())
                .unwrap_or_else(|| String::from("Mutant")),
            replacement: self.replacement,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum CargoMutantScenario {
    Tagged {
        #[serde(rename = "Mutant")]
        mutant: Box<CargoMutantDetail>,
    },
    #[serde(deserialize_with = "deserialize_ignored")]
    Other,
}

fn deserialize_ignored<'de, D: serde::Deserializer<'de>>(d: D) -> Result<(), D::Error> {
    serde::de::IgnoredAny::deserialize(d).map(|_| ())
}

#[derive(Deserialize)]
pub struct CargoMutantOutcome {
    pub summary: Option<String>,
    pub scenario: Option<CargoMutantScenario>,
}

#[derive(Deserialize)]
pub struct CargoMutantsReport {
    #[serde(default)]
    pub outcomes: Vec<CargoMutantOutcome>,
}

#[derive(Deserialize)]
pub struct StrykerMutant {
    #[serde(rename = "mutatorName")]
    mutator_name: Option<String>,
    replacement: Option<String>,
    description: Option<String>,
    pub status: Option<String>,
    location: Option<RawMutantSpan>,
    function_name: Option<String>,
}

impl StrykerMutant {
    pub fn into_normalized(self, file_key: &str) -> NormalizedRawSurvivor {
        NormalizedRawSurvivor {
            raw_uri: Some(String::from(file_key)),
            fallback_path: "package.json",
            span: self
                .location
                .as_ref()
                .map_or(Span::FIRST, RawMutantSpan::to_span),
            function_name: self.function_name.filter(|s| !s.trim().is_empty()),
            mutator: self
                .mutator_name
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| String::from("Mutator")),
            replacement: self.replacement.or(self.description),
        }
    }
}

#[derive(Deserialize)]
pub struct StrykerFileEntry {
    #[serde(default)]
    pub mutants: Vec<StrykerMutant>,
}

#[derive(Deserialize)]
pub struct StrykerReport {
    #[serde(default)]
    pub files: BTreeMap<String, StrykerFileEntry>,
}
