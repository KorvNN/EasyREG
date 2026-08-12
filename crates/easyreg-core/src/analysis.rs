use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Dialect, GeneralizationStrategy, PatternSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteCode {
    FieldClassified,
    ObservedLengthRange,
    FlexibleLength,
    ExactAlternationFallback,
    CommonAffixFallback,
    SingleExampleLowConfidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InferenceNote {
    pub code: NoteCode,
    pub field_id: Option<String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExampleMatch {
    pub input: String,
    pub matched: bool,
    #[serde(default)]
    pub captures: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub positive_results: Vec<ExampleMatch>,
    pub negative_results: Vec<ExampleMatch>,
    pub positive_coverage: f64,
    pub negative_rejection: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalyzedCandidate {
    pub strategy: GeneralizationStrategy,
    pub spec: PatternSpec,
    pub renderings: BTreeMap<Dialect, String>,
    pub validation: ValidationReport,
    pub score: f64,
    pub notes: Vec<InferenceNote>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub candidates: Vec<AnalyzedCandidate>,
    pub recommended_strategy: GeneralizationStrategy,
}
