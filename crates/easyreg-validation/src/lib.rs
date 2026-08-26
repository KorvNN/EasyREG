//! Safe evaluation of generated portable patterns against supplied examples.

use std::collections::BTreeMap;

use easyreg_core::{Dialect, ExampleMatch, PatternSpec, ValidationReport};
use easyreg_dialects::{RenderError, render};
use regex::Regex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error(transparent)]
    Render(#[from] RenderError),
    #[error("the generated safe-evaluator pattern did not compile: {0}")]
    Compile(#[from] regex::Error),
}

/// Evaluates a pattern against all positive and negative examples.
///
/// # Errors
///
/// Returns [`ValidationError`] when the internal Rust-dialect pattern cannot
/// be rendered or compiled.
pub fn validate(
    spec: &PatternSpec,
    positive_examples: &[String],
    negative_examples: &[String],
) -> Result<ValidationReport, ValidationError> {
    let pattern = render(spec, Dialect::Rust)?;
    let regex = Regex::new(&pattern)?;

    let positive_results = positive_examples
        .iter()
        .map(|input| evaluate(&regex, input))
        .collect::<Vec<_>>();
    let negative_results = negative_examples
        .iter()
        .map(|input| evaluate(&regex, input))
        .collect::<Vec<_>>();

    let positive_coverage = ratio(
        positive_results
            .iter()
            .filter(|result| result.matched)
            .count(),
        positive_results.len(),
        0.0,
    );
    let negative_rejection = ratio(
        negative_results
            .iter()
            .filter(|result| !result.matched)
            .count(),
        negative_results.len(),
        1.0,
    );

    Ok(ValidationReport {
        positive_results,
        negative_results,
        positive_coverage,
        negative_rejection,
    })
}

fn evaluate(regex: &Regex, input: &str) -> ExampleMatch {
    let Some(captures) = regex.captures(input) else {
        return ExampleMatch {
            input: input.to_owned(),
            matched: false,
            captures: BTreeMap::new(),
        };
    };

    let values = regex
        .capture_names()
        .flatten()
        .filter_map(|name| {
            captures
                .name(name)
                .map(|value| (name.to_owned(), value.as_str().to_owned()))
        })
        .collect::<BTreeMap<_, _>>();

    ExampleMatch {
        input: input.to_owned(),
        matched: true,
        captures: values,
    }
}

#[allow(clippy::cast_precision_loss)]
fn ratio(numerator: usize, denominator: usize, empty_value: f64) -> f64 {
    if denominator == 0 {
        empty_value
    } else {
        numerator as f64 / denominator as f64
    }
}

#[cfg(test)]
mod tests {
    use easyreg_core::{FieldKind, FieldSpec, MatchMode, PatternNode};

    use super::*;

    #[test]
    fn reports_coverage_rejection_and_named_captures() {
        let spec = PatternSpec::new(
            MatchMode::Full,
            PatternNode::Sequence {
                nodes: vec![
                    PatternNode::Literal {
                        value: "ORD-".to_owned(),
                    },
                    PatternNode::Field {
                        field: FieldSpec {
                            id: "order_id".to_owned(),
                            name: None,
                            kind: FieldKind::Integer,
                            min_length: 5,
                            max_length: Some(5),
                            capture: true,
                        },
                    },
                ],
            },
        );
        let report = validate(
            &spec,
            &["ORD-00127".to_owned(), "ORD-84621".to_owned()],
            &["ORD-12".to_owned(), "INV-00127".to_owned()],
        )
        .expect("validation should succeed");

        assert!((report.positive_coverage - 1.0).abs() < f64::EPSILON);
        assert!((report.negative_rejection - 1.0).abs() < f64::EPSILON);
        assert_eq!(
            report.positive_results[0].captures.get("order_id"),
            Some(&"00127".to_owned())
        );
    }
}
