//! Example-driven construction of engine-neutral pattern specifications.

use std::collections::BTreeMap;

use easyreg_core::{
    AnalyzeRequest, FieldKind, FieldSpec, GeneralizationStrategy, InferenceNote, InputError,
    MatchMode, NoteCode, PatternNode, PatternSpec,
};
use easyreg_detectors::DetectorRegistry;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferredCandidate {
    pub strategy: GeneralizationStrategy,
    pub spec: PatternSpec,
    pub notes: Vec<InferenceNote>,
}

#[derive(Debug, Error)]
pub enum InferenceError {
    #[error(transparent)]
    InvalidInput(#[from] InputError),
}

/// Builds strict, balanced, and flexible candidates from the supplied examples.
///
/// # Errors
///
/// Returns [`InferenceError::InvalidInput`] when the request has no usable
/// positive examples.
pub fn infer(request: &AnalyzeRequest) -> Result<Vec<InferredCandidate>, InferenceError> {
    request.validate()?;

    let registry = DetectorRegistry::default();
    let positive_refs = request
        .positive_examples
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let full_kind = registry.classify_all(&positive_refs);
    let segments = if full_kind.has_intrinsic_shape() {
        Some(vec![Segment::Variable(request.positive_examples.clone())])
    } else if request.positive_examples.len() == 1 {
        infer_single_example(&request.positive_examples[0])
    } else {
        infer_aligned_segments(&request.positive_examples)
    };

    let strategies = [
        GeneralizationStrategy::Strict,
        GeneralizationStrategy::Balanced,
        GeneralizationStrategy::Flexible,
    ];

    let candidates = match segments {
        Some(segments) => strategies
            .into_iter()
            .map(|strategy| {
                build_from_segments(
                    &segments,
                    strategy,
                    request.match_mode,
                    &registry,
                    request.positive_examples.len() == 1,
                )
            })
            .collect(),
        None => build_fallback_candidates(request, &registry),
    };

    Ok(candidates)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Digits,
    Uppercase,
    Lowercase,
    Alphabetic,
    Whitespace,
    Punctuation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    text: String,
    kind: TokenKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Literal(String),
    Variable(Vec<String>),
}

fn infer_single_example(example: &str) -> Option<Vec<Segment>> {
    let registry = DetectorRegistry::default();
    let full_kind = registry.classify_all(&[example]);
    if full_kind.has_intrinsic_shape()
        || matches!(
            full_kind,
            FieldKind::Integer | FieldKind::Decimal | FieldKind::Hexadecimal
        )
    {
        return Some(vec![Segment::Variable(vec![example.to_owned()])]);
    }

    let mut segments = Vec::new();
    for token in tokenize(example) {
        if token.kind == TokenKind::Digits {
            push_segment(&mut segments, Segment::Variable(vec![token.text]));
        } else {
            push_segment(&mut segments, Segment::Literal(token.text));
        }
    }

    (!segments.is_empty()).then_some(segments)
}

fn infer_aligned_segments(examples: &[String]) -> Option<Vec<Segment>> {
    let tokenized = examples
        .iter()
        .map(|example| tokenize(example))
        .collect::<Vec<_>>();
    let token_count = tokenized.first()?.len();

    if token_count == 0 || tokenized.iter().any(|tokens| tokens.len() != token_count) {
        return None;
    }

    let mut segments = Vec::new();
    for column in 0..token_count {
        let values = tokenized
            .iter()
            .map(|tokens| tokens[column].text.clone())
            .collect::<Vec<_>>();

        if values.windows(2).all(|pair| pair[0] == pair[1]) {
            push_segment(&mut segments, Segment::Literal(values[0].clone()));
        } else {
            push_segment(&mut segments, Segment::Variable(values));
        }
    }

    Some(segments)
}

fn push_segment(segments: &mut Vec<Segment>, incoming: Segment) {
    match (segments.last_mut(), incoming) {
        (Some(Segment::Literal(existing)), Segment::Literal(value)) => existing.push_str(&value),
        (Some(Segment::Variable(existing)), Segment::Variable(values))
            if existing.len() == values.len()
                && !all_whitespace(existing)
                && !all_whitespace(&values) =>
        {
            for (current, value) in existing.iter_mut().zip(values) {
                current.push_str(&value);
            }
        }
        (_, incoming) => segments.push(incoming),
    }
}

fn all_whitespace(values: &[String]) -> bool {
    values
        .iter()
        .all(|value| value.chars().all(char::is_whitespace))
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut active_kind = None;

    for (index, character) in input.char_indices() {
        let kind = token_kind(character);
        if active_kind.is_some_and(|active| active != kind) {
            tokens.push(Token {
                text: input[start..index].to_owned(),
                kind: active_kind.expect("an active token kind must exist"),
            });
            start = index;
        }
        active_kind = Some(kind);
    }

    if let Some(kind) = active_kind {
        tokens.push(Token {
            text: input[start..].to_owned(),
            kind,
        });
    }

    tokens
}

fn token_kind(character: char) -> TokenKind {
    if character.is_ascii_digit() {
        TokenKind::Digits
    } else if character.is_ascii_uppercase() {
        TokenKind::Uppercase
    } else if character.is_ascii_lowercase() {
        TokenKind::Lowercase
    } else if character.is_alphabetic() {
        TokenKind::Alphabetic
    } else if character.is_whitespace() {
        TokenKind::Whitespace
    } else {
        TokenKind::Punctuation
    }
}

fn build_from_segments(
    segments: &[Segment],
    strategy: GeneralizationStrategy,
    match_mode: MatchMode,
    registry: &DetectorRegistry,
    single_example: bool,
) -> InferredCandidate {
    let mut pattern_nodes = Vec::new();
    let mut notes = Vec::new();
    let mut field_number = 0;

    if single_example {
        notes.push(note(NoteCode::SingleExampleLowConfidence, None, []));
    }

    for segment in segments {
        match segment {
            Segment::Literal(value) => pattern_nodes.push(PatternNode::Literal {
                value: value.clone(),
            }),
            Segment::Variable(values) => {
                field_number += 1;
                let field_id = format!("field_{field_number}");
                let references = values.iter().map(String::as_str).collect::<Vec<_>>();
                let kind = registry.classify_all(&references);
                let observed_min = values.iter().map(|value| value.chars().count()).min().unwrap_or(0);
                let observed_max = values.iter().map(|value| value.chars().count()).max().unwrap_or(0);
                let (min_length, max_length) = length_constraints(
                    kind,
                    observed_min,
                    observed_max,
                    strategy,
                );

                notes.push(note(
                    NoteCode::FieldClassified,
                    Some(&field_id),
                    [("kind", kind.as_str().to_owned())],
                ));
                notes.push(note(
                    NoteCode::ObservedLengthRange,
                    Some(&field_id),
                    [
                        ("min", observed_min.to_string()),
                        ("max", observed_max.to_string()),
                    ],
                ));
                if strategy == GeneralizationStrategy::Flexible && !kind.has_intrinsic_shape() {
                    notes.push(note(
                        NoteCode::FlexibleLength,
                        Some(&field_id),
                        [("min", min_length.to_string()), ("max", "unbounded".to_owned())],
                    ));
                }

                pattern_nodes.push(PatternNode::Field {
                    field: FieldSpec {
                        id: field_id,
                        name: None,
                        kind,
                        min_length,
                        max_length,
                        capture: kind != FieldKind::Whitespace,
                    },
                });
            }
        }
    }

    InferredCandidate {
        strategy,
        spec: PatternSpec::new(match_mode, sequence(pattern_nodes)),
        notes,
    }
}

fn length_constraints(
    kind: FieldKind,
    observed_min: usize,
    observed_max: usize,
    strategy: GeneralizationStrategy,
) -> (usize, Option<usize>) {
    if kind.has_intrinsic_shape() {
        return (observed_min, Some(observed_max));
    }

    match strategy {
        GeneralizationStrategy::Strict => (observed_min, Some(observed_max)),
        GeneralizationStrategy::Balanced if observed_min == observed_max => {
            (observed_min, Some(observed_max))
        }
        GeneralizationStrategy::Balanced => (observed_min, None),
        GeneralizationStrategy::Flexible => (usize::from(observed_min > 0), None),
    }
}

fn build_fallback_candidates(
    request: &AnalyzeRequest,
    registry: &DetectorRegistry,
) -> Vec<InferredCandidate> {
    let strict = InferredCandidate {
        strategy: GeneralizationStrategy::Strict,
        spec: PatternSpec::new(
            request.match_mode,
            PatternNode::Alternation {
                branches: deduplicated_literals(&request.positive_examples),
            },
        ),
        notes: vec![note(NoteCode::ExactAlternationFallback, None, [])],
    };

    let prefix = longest_common_prefix(&request.positive_examples);
    let suffix = longest_common_suffix(&request.positive_examples, prefix.chars().count());
    let middle_values = request
        .positive_examples
        .iter()
        .map(|example| {
            example[prefix.len()..example.len().saturating_sub(suffix.len())].to_owned()
        })
        .collect::<Vec<_>>();

    let fallback_segments = [
        (!prefix.is_empty()).then_some(Segment::Literal(prefix)),
        Some(Segment::Variable(middle_values)),
        (!suffix.is_empty()).then_some(Segment::Literal(suffix)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    let mut balanced = build_from_segments(
        &fallback_segments,
        GeneralizationStrategy::Balanced,
        request.match_mode,
        registry,
        false,
    );
    balanced
        .notes
        .push(note(NoteCode::CommonAffixFallback, None, []));

    let mut flexible = build_from_segments(
        &fallback_segments,
        GeneralizationStrategy::Flexible,
        request.match_mode,
        registry,
        false,
    );
    flexible
        .notes
        .push(note(NoteCode::CommonAffixFallback, None, []));

    vec![strict, balanced, flexible]
}

fn deduplicated_literals(examples: &[String]) -> Vec<PatternNode> {
    let mut values = examples.to_vec();
    values.sort();
    values.dedup();
    values
        .into_iter()
        .map(|value| PatternNode::Literal { value })
        .collect()
}

fn longest_common_prefix(examples: &[String]) -> String {
    let Some(first) = examples.first() else {
        return String::new();
    };

    first
        .chars()
        .enumerate()
        .take_while(|(index, character)| {
            examples[1..]
                .iter()
                .all(|example| example.chars().nth(*index) == Some(*character))
        })
        .map(|(_, character)| character)
        .collect()
}

fn longest_common_suffix(examples: &[String], prefix_length: usize) -> String {
    let Some(first) = examples.first() else {
        return String::new();
    };
    let remaining = examples
        .iter()
        .map(|example| example.chars().count().saturating_sub(prefix_length))
        .min()
        .unwrap_or(0);

    let reversed = first
        .chars()
        .rev()
        .take(remaining)
        .enumerate()
        .take_while(|(index, character)| {
            examples[1..]
                .iter()
                .all(|example| example.chars().rev().nth(*index) == Some(*character))
        })
        .map(|(_, character)| character)
        .collect::<Vec<_>>();

    reversed.into_iter().rev().collect()
}

fn sequence(mut nodes: Vec<PatternNode>) -> PatternNode {
    if nodes.len() == 1 {
        nodes.pop().expect("one node must be present")
    } else {
        PatternNode::Sequence { nodes }
    }
}

fn note<const N: usize>(
    code: NoteCode,
    field_id: Option<&str>,
    attributes: [(&str, String); N],
) -> InferenceNote {
    InferenceNote {
        code,
        field_id: field_id.map(str::to_owned),
        attributes: attributes
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect::<BTreeMap<_, _>>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(positives: &[&str]) -> AnalyzeRequest {
        AnalyzeRequest {
            positive_examples: positives.iter().map(|value| (*value).to_owned()).collect(),
            negative_examples: Vec::new(),
            match_mode: MatchMode::Full,
        }
    }

    #[test]
    fn infers_literals_and_two_numeric_fields() {
        let candidates = infer(&request(&[
            "INV-2026-00127",
            "INV-2025-84621",
            "INV-2026-18342",
        ]))
        .expect("inference should succeed");

        let PatternNode::Sequence { nodes } = &candidates[0].spec.root else {
            panic!("a sequence was expected");
        };
        let field_count = nodes
            .iter()
            .filter(|node| matches!(node, PatternNode::Field { .. }))
            .count();

        assert_eq!(field_count, 2);
        assert!(matches!(
            &nodes[0],
            PatternNode::Literal { value } if value == "INV-"
        ));
    }

    #[test]
    fn recognizes_a_single_ipv4_value() {
        let candidates = infer(&request(&["192.168.10.20"])).expect("inference should succeed");

        assert!(matches!(
            &candidates[1].spec.root,
            PatternNode::Field { field } if field.kind == FieldKind::Ipv4
        ));
    }

    #[test]
    fn uses_an_exact_alternation_when_token_shapes_do_not_align() {
        let candidates = infer(&request(&["alpha-12", "completely different value"]))
            .expect("inference should succeed");

        assert!(matches!(
            candidates[0].spec.root,
            PatternNode::Alternation { .. }
        ));
    }
}
