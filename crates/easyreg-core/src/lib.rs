//! Shared, provider-independent domain types for `EasyREG`.

mod analysis;
mod pattern;
mod request;

pub use analysis::{
    AnalysisResult, AnalyzedCandidate, ExampleMatch, InferenceNote, NoteCode, ValidationReport,
};
pub use pattern::{
    Dialect, FieldKind, FieldSpec, GeneralizationStrategy, MatchMode, PatternNode, PatternSpec,
};
pub use request::{AnalyzeRequest, InputError};
