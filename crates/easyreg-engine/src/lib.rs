//! Application service that composes inference, rendering, and validation.

use std::collections::BTreeMap;

use easyreg_core::{
    AnalysisResult, AnalyzeRequest, AnalyzedCandidate, Dialect, GeneralizationStrategy,
    ValidationReport,
};
use easyreg_dialects::{RenderError, render};
use easyreg_inference::{InferenceError, infer};
use easyreg_validation::{ValidationError, validate};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error(transparent)]
    Inference(#[from] InferenceError),
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error(transparent)]
    Validation(#[from] ValidationError),
}

/// Runs inference, validation, scoring, and dialect rendering as one operation.
///
/// # Errors
///
/// Returns [`AnalysisError`] when the request is invalid, a candidate cannot be
/// rendered, or the internal validation expression cannot be compiled.
pub fn analyze(request: &AnalyzeRequest) -> Result<AnalysisResult, AnalysisError> {
    let inferred = infer(request)?;
    let mut candidates = Vec::with_capacity(inferred.len());

    for candidate in inferred {
        let validation = validate(
            &candidate.spec,
            &request.positive_examples,
            &request.negative_examples,
        )?;
        let renderings = [Dialect::JavaScript, Dialect::Python, Dialect::Pcre2]
            .into_iter()
            .map(|dialect| render(&candidate.spec, dialect).map(|pattern| (dialect, pattern)))
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let score = score(candidate.strategy, &validation, candidate.spec.root.node_count());

        candidates.push(AnalyzedCandidate {
            strategy: candidate.strategy,
            spec: candidate.spec,
            renderings,
            validation,
            score,
            notes: candidate.notes,
        });
    }

    let recommended_strategy = candidates
        .iter()
        .max_by(|left, right| left.score.total_cmp(&right.score))
        .map_or(GeneralizationStrategy::Balanced, |candidate| {
            candidate.strategy
        });

    Ok(AnalysisResult {
        candidates,
        recommended_strategy,
    })
}

#[allow(clippy::cast_precision_loss)]
fn score(
    strategy: GeneralizationStrategy,
    validation: &ValidationReport,
    node_count: usize,
) -> f64 {
    let strategy_prior = match strategy {
        GeneralizationStrategy::Strict => 0.8,
        GeneralizationStrategy::Balanced => 1.0,
        GeneralizationStrategy::Flexible => 0.6,
    };
    let simplicity = 1.0 / (1.0 + node_count as f64 / 20.0);

    0.65 * validation.positive_coverage
        + 0.25 * validation.negative_rejection
        + 0.07 * strategy_prior
        + 0.03 * simplicity
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use easyreg_core::MatchMode;
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    struct GoldenCase {
        request: AnalyzeRequest,
        expected_balanced_javascript: String,
    }

    #[test]
    fn passes_the_invoice_id_golden_case() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/invoice_ids.json");
        let contents = fs::read_to_string(path).expect("golden case should be readable");
        let case: GoldenCase =
            serde_json::from_str(&contents).expect("golden case should be valid JSON");

        let result = analyze(&case.request).expect("analysis should succeed");
        let balanced = result
            .candidates
            .iter()
            .find(|candidate| candidate.strategy == GeneralizationStrategy::Balanced)
            .expect("balanced candidate should exist");

        assert!((balanced.validation.positive_coverage - 1.0).abs() < f64::EPSILON);
        assert!((balanced.validation.negative_rejection - 1.0).abs() < f64::EPSILON);
        assert_eq!(
            balanced.renderings[&Dialect::JavaScript],
            case.expected_balanced_javascript
        );
        assert_eq!(result.recommended_strategy, GeneralizationStrategy::Balanced);
    }

    #[test]
    fn rejects_an_empty_request() {
        let error = analyze(&AnalyzeRequest {
            positive_examples: Vec::new(),
            negative_examples: Vec::new(),
            match_mode: MatchMode::Full,
        })
        .expect_err("an empty request should fail");

        assert!(error.to_string().contains("positive example"));
    }
}
