//! Application service that composes inference, rendering, and validation.

use std::collections::BTreeMap;

use easyreg_core::{
    AnalysisResult, AnalyzeRequest, AnalyzedCandidate, Dialect, GeneralizationStrategy,
    PatternNode, ValidationReport,
};
use easyreg_dialects::{RenderError, render};
use easyreg_inference::{InferenceError, InferredCandidate, infer, infer_exact};
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
        candidates.push(analyze_candidate(candidate, request)?);
    }

    if let Some(strict_index) = candidates
        .iter()
        .position(|candidate| candidate.strategy == GeneralizationStrategy::Strict)
        && candidates[strict_index].validation.negative_rejection < 1.0
    {
        let exact = analyze_candidate(infer_exact(request)?, request)?;
        if exact.validation.negative_rejection
            > candidates[strict_index].validation.negative_rejection
        {
            candidates[strict_index] = exact;
        }
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

/// Applies validated semantic names to inferred capture fields and refreshes
/// every rendering and validation report.
///
/// Unknown field identifiers are ignored so that structurally different
/// candidates can coexist in one analysis result.
///
/// # Errors
///
/// Returns [`AnalysisError`] if a renamed pattern cannot be rendered or
/// validated.
pub fn apply_field_names(
    result: &mut AnalysisResult,
    request: &AnalyzeRequest,
    names: &BTreeMap<String, String>,
) -> Result<(), AnalysisError> {
    for candidate in &mut result.candidates {
        rename_node_fields(&mut candidate.spec.root, names);
        candidate.validation = validate(
            &candidate.spec,
            &request.positive_examples,
            &request.negative_examples,
        )?;
        candidate.renderings = [Dialect::JavaScript, Dialect::Python, Dialect::Pcre2]
            .into_iter()
            .map(|dialect| render(&candidate.spec, dialect).map(|pattern| (dialect, pattern)))
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        candidate.score = score(
            candidate.strategy,
            &candidate.validation,
            candidate.spec.root.node_count(),
        );
    }

    Ok(())
}

fn rename_node_fields(node: &mut PatternNode, names: &BTreeMap<String, String>) {
    match node {
        PatternNode::Field { field } => {
            if let Some(name) = names.get(&field.id) {
                field.name = Some(name.clone());
            }
        }
        PatternNode::Sequence { nodes } => {
            for node in nodes {
                rename_node_fields(node, names);
            }
        }
        PatternNode::Alternation { branches } => {
            for branch in branches {
                rename_node_fields(branch, names);
            }
        }
        PatternNode::Repeat { node, .. } => rename_node_fields(node, names),
        PatternNode::Literal { .. } => {}
    }
}

fn analyze_candidate(
    candidate: InferredCandidate,
    request: &AnalyzeRequest,
) -> Result<AnalyzedCandidate, AnalysisError> {
    let validation = validate(
        &candidate.spec,
        &request.positive_examples,
        &request.negative_examples,
    )?;
    let renderings = [Dialect::JavaScript, Dialect::Python, Dialect::Pcre2]
        .into_iter()
        .map(|dialect| render(&candidate.spec, dialect).map(|pattern| (dialect, pattern)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let score = score(
        candidate.strategy,
        &validation,
        candidate.spec.root.node_count(),
    );

    Ok(AnalyzedCandidate {
        strategy: candidate.strategy,
        spec: candidate.spec,
        renderings,
        validation,
        score,
        notes: candidate.notes,
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

    use easyreg_core::{MatchMode, NoteCode, PatternNode};
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize)]
    struct GoldenCase {
        request: AnalyzeRequest,
        expected_balanced_javascript: String,
    }

    #[test]
    fn passes_the_invoice_id_golden_case() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/invoice_ids.json");
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
        assert_eq!(
            result.recommended_strategy,
            GeneralizationStrategy::Balanced
        );
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

    #[test]
    fn rejects_impossible_iso_calendar_dates() {
        let result = analyze(&AnalyzeRequest {
            positive_examples: vec!["2024-02-29".to_owned(), "2026-08-12".to_owned()],
            negative_examples: vec![
                "2023-02-29".to_owned(),
                "1900-02-29".to_owned(),
                "2026-04-31".to_owned(),
            ],
            match_mode: MatchMode::Full,
        })
        .expect("date analysis should succeed");

        assert!(result.candidates.iter().all(|candidate| {
            (candidate.validation.positive_coverage - 1.0).abs() < f64::EPSILON
                && (candidate.validation.negative_rejection - 1.0).abs() < f64::EPSILON
        }));
    }

    #[test]
    fn replaces_a_broad_strict_candidate_with_an_exact_negative_aware_fallback() {
        let result = analyze(&AnalyzeRequest {
            positive_examples: vec!["AB12".to_owned(), "CD34".to_owned()],
            negative_examples: vec!["EF56".to_owned()],
            match_mode: MatchMode::Full,
        })
        .expect("analysis should succeed");
        let strict = result
            .candidates
            .iter()
            .find(|candidate| candidate.strategy == GeneralizationStrategy::Strict)
            .expect("strict candidate should exist");

        assert!(matches!(strict.spec.root, PatternNode::Alternation { .. }));
        assert!((strict.validation.negative_rejection - 1.0).abs() < f64::EPSILON);
        assert!(
            strict
                .notes
                .iter()
                .any(|note| note.code == NoteCode::ExactAlternationFallback)
        );
        assert_eq!(result.recommended_strategy, GeneralizationStrategy::Strict);
    }

    #[test]
    fn retains_the_structured_strict_candidate_when_exact_matching_cannot_improve_rejection() {
        let result = analyze(&AnalyzeRequest {
            positive_examples: vec!["AB12".to_owned(), "CD34".to_owned()],
            negative_examples: vec!["AB12".to_owned()],
            match_mode: MatchMode::Full,
        })
        .expect("analysis should succeed");
        let strict = result
            .candidates
            .iter()
            .find(|candidate| candidate.strategy == GeneralizationStrategy::Strict)
            .expect("strict candidate should exist");

        assert!(matches!(strict.spec.root, PatternNode::Field { .. }));
        assert!(
            strict
                .notes
                .iter()
                .all(|note| note.code != NoteCode::ExactAlternationFallback)
        );
    }

    #[test]
    fn applies_semantic_capture_names_and_refreshes_captures() {
        let request = AnalyzeRequest {
            positive_examples: vec!["INV-2026-00127".to_owned(), "INV-2025-84621".to_owned()],
            negative_examples: Vec::new(),
            match_mode: MatchMode::Full,
        };
        let mut result = analyze(&request).expect("analysis should succeed");
        apply_field_names(
            &mut result,
            &request,
            &BTreeMap::from([
                ("field_1".to_owned(), "year".to_owned()),
                ("field_2".to_owned(), "invoice_id".to_owned()),
            ]),
        )
        .expect("semantic names should render");
        let balanced = result
            .candidates
            .iter()
            .find(|candidate| candidate.strategy == GeneralizationStrategy::Balanced)
            .expect("balanced candidate should exist");

        assert!(balanced.renderings[&Dialect::JavaScript].contains("(?<year>"));
        assert_eq!(
            balanced.validation.positive_results[0]
                .captures
                .get("invoice_id"),
            Some(&"00127".to_owned())
        );
    }
}
