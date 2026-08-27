//! Rendering of a `PatternSpec` into concrete regex dialects.

use std::collections::HashSet;

use easyreg_core::{Dialect, FieldKind, FieldSpec, MatchMode, PatternNode, PatternSpec};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialectFeature {
    NamedCaptures,
    Lookaround,
    Lookbehind,
    Backreferences,
    AtomicGroups,
}

pub const fn capabilities(dialect: Dialect) -> &'static [DialectFeature] {
    match dialect {
        Dialect::JavaScript => &[
            DialectFeature::NamedCaptures,
            DialectFeature::Lookaround,
            DialectFeature::Lookbehind,
            DialectFeature::Backreferences,
        ],
        Dialect::Python | Dialect::Pcre2 => &[
            DialectFeature::NamedCaptures,
            DialectFeature::Lookaround,
            DialectFeature::Lookbehind,
            DialectFeature::Backreferences,
            DialectFeature::AtomicGroups,
        ],
        Dialect::Rust => &[DialectFeature::NamedCaptures],
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RenderError {
    #[error("field '{field_id}' has an invalid length range: {min}..={max}")]
    InvalidLengthRange {
        field_id: String,
        min: usize,
        max: usize,
    },
    #[error("'{name}' is not a portable capture name")]
    InvalidCaptureName { name: String },
    #[error("capture name '{name}' is used more than once")]
    DuplicateCaptureName { name: String },
    #[error("repeat node has an invalid range: {min}..={max}")]
    InvalidRepeatRange { min: usize, max: usize },
}

/// Converts an engine-neutral pattern into a concrete regex dialect.
///
/// # Errors
///
/// Returns [`RenderError`] when a field or repetition has an invalid range,
/// or when capture names are invalid or duplicated.
pub fn render(spec: &PatternSpec, dialect: Dialect) -> Result<String, RenderError> {
    let mut state = RenderState {
        dialect,
        capture_names: HashSet::new(),
    };
    let body = state.render_node(&spec.root)?;

    Ok(match (spec.match_mode, dialect) {
        (MatchMode::Search, _) => body,
        (MatchMode::Full, Dialect::JavaScript) => format!("^(?:{body})$"),
        (MatchMode::Full, Dialect::Python) => format!(r"\A(?:{body})\Z"),
        (MatchMode::Full, Dialect::Pcre2 | Dialect::Rust) => format!(r"\A(?:{body})\z"),
    })
}

struct RenderState {
    dialect: Dialect,
    capture_names: HashSet<String>,
}

impl RenderState {
    fn render_node(&mut self, node: &PatternNode) -> Result<String, RenderError> {
        match node {
            PatternNode::Literal { value } => Ok(escape_literal(value)),
            PatternNode::Field { field } => self.render_field(field),
            PatternNode::Sequence { nodes } => nodes
                .iter()
                .map(|node| self.render_node(node))
                .collect::<Result<String, _>>(),
            PatternNode::Alternation { branches } => {
                let branches = branches
                    .iter()
                    .map(|branch| self.render_node(branch))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(format!("(?:{})", branches.join("|")))
            }
            PatternNode::Repeat { node, min, max } => {
                let body = self.render_node(node)?;
                let quantifier = quantifier(*min, *max)
                    .map_err(|max| RenderError::InvalidRepeatRange { min: *min, max })?;
                Ok(format!("(?:{body}){quantifier}"))
            }
        }
    }

    fn render_field(&mut self, field: &FieldSpec) -> Result<String, RenderError> {
        let body = field_body(field)?;
        if !field.capture {
            return Ok(body);
        }

        let name = field.capture_name();
        if !valid_capture_name(name) {
            return Err(RenderError::InvalidCaptureName {
                name: name.to_owned(),
            });
        }
        if !self.capture_names.insert(name.to_owned()) {
            return Err(RenderError::DuplicateCaptureName {
                name: name.to_owned(),
            });
        }

        Ok(match self.dialect {
            Dialect::JavaScript | Dialect::Pcre2 => format!("(?<{name}>{body})"),
            Dialect::Python | Dialect::Rust => format!("(?P<{name}>{body})"),
        })
    }
}

fn field_body(field: &FieldSpec) -> Result<String, RenderError> {
    let semantic = match field.kind {
        FieldKind::Ipv4 => Some(
            r"(?:25[0-5]|2[0-4][0-9]|1?[0-9]{1,2})(?:\.(?:25[0-5]|2[0-4][0-9]|1?[0-9]{1,2})){3}",
        ),
        // This portable form deliberately avoids lookaround and backreferences.
        FieldKind::Ipv6 => Some(r"(?:[0-9A-Fa-f]{0,4}:){1,7}[0-9A-Fa-f]{0,4}"),
        FieldKind::Uuid => {
            Some(r"[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}")
        }
        FieldKind::Email => Some(r"[^@\s]+@[^@\s]+\.[^@\s]+"),
        FieldKind::Url => Some(r"https?://[^\s]+"),
        FieldKind::Path => Some(r"/[^\s]*"),
        FieldKind::DateIso => Some(concat!(
            r"(?:[0-9]{4}-(?:",
            r"(?:0[13578]|1[02])-(?:0[1-9]|[12][0-9]|3[01])|",
            r"(?:0[469]|11)-(?:0[1-9]|[12][0-9]|30)|",
            r"02-(?:0[1-9]|1[0-9]|2[0-8])",
            r")|",
            r"(?:[0-9]{2}(?:0[48]|[2468][048]|[13579][26])|",
            r"(?:[02468][048]|[13579][26])00)-02-29",
            r")",
        )),
        FieldKind::Time => Some(r"(?:[01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9](?:\.[0-9]+)?"),
        FieldKind::Decimal => Some(r"[0-9]+\.[0-9]+"),
        FieldKind::Hexadecimal => Some(r"0[xX][0-9A-Fa-f]+"),
        _ => None,
    };

    if let Some(pattern) = semantic {
        return Ok(pattern.to_owned());
    }

    let atom = match field.kind {
        FieldKind::Integer => "[0-9]",
        FieldKind::Uppercase => "[A-Z]",
        FieldKind::Lowercase => "[a-z]",
        FieldKind::Alphabetic => "[A-Za-z]",
        FieldKind::Alphanumeric => "[A-Za-z0-9]",
        FieldKind::Whitespace => r"\s",
        FieldKind::NonWhitespace => r"\S",
        FieldKind::Text => r"[\s\S]",
        FieldKind::Ipv4
        | FieldKind::Ipv6
        | FieldKind::Uuid
        | FieldKind::Email
        | FieldKind::Url
        | FieldKind::Path
        | FieldKind::DateIso
        | FieldKind::Time
        | FieldKind::Decimal
        | FieldKind::Hexadecimal => unreachable!("semantic fields returned above"),
    };

    let quantifier = quantifier(field.min_length, field.max_length).map_err(|max| {
        RenderError::InvalidLengthRange {
            field_id: field.id.clone(),
            min: field.min_length,
            max,
        }
    })?;
    Ok(format!("{atom}{quantifier}"))
}

fn quantifier(min: usize, max: Option<usize>) -> Result<String, usize> {
    if let Some(max) = max
        && min > max
    {
        return Err(max);
    }

    Ok(match (min, max) {
        (0, None) => "*".to_owned(),
        (1, None) => "+".to_owned(),
        (min, None) => format!("{{{min},}}"),
        (0, Some(1)) => "?".to_owned(),
        (1, Some(1)) => String::new(),
        (min, Some(max)) if min == max => format!("{{{min}}}"),
        (min, Some(max)) => format!("{{{min},{max}}}"),
    })
}

fn escape_literal(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(
            character,
            '\\' | '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn valid_capture_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invoice_spec() -> PatternSpec {
        PatternSpec::new(
            MatchMode::Full,
            PatternNode::Sequence {
                nodes: vec![
                    PatternNode::Literal {
                        value: "INV-".to_owned(),
                    },
                    PatternNode::Field {
                        field: FieldSpec {
                            id: "year".to_owned(),
                            name: None,
                            kind: FieldKind::Integer,
                            min_length: 4,
                            max_length: Some(4),
                            capture: true,
                        },
                    },
                ],
            },
        )
    }

    #[test]
    fn renders_dialect_specific_named_captures_and_anchors() {
        let spec = invoice_spec();

        assert_eq!(
            render(&spec, Dialect::JavaScript).expect("JavaScript rendering should work"),
            r"^(?:INV-(?<year>[0-9]{4}))$"
        );
        assert_eq!(
            render(&spec, Dialect::Python).expect("Python rendering should work"),
            r"\A(?:INV-(?P<year>[0-9]{4}))\Z"
        );
        assert_eq!(
            render(&spec, Dialect::Pcre2).expect("PCRE2 rendering should work"),
            r"\A(?:INV-(?<year>[0-9]{4}))\z"
        );
    }

    #[test]
    fn escapes_literal_regex_metacharacters() {
        let spec = PatternSpec::new(
            MatchMode::Search,
            PatternNode::Literal {
                value: "price: $12.50".to_owned(),
            },
        );

        assert_eq!(
            render(&spec, Dialect::Rust).expect("rendering should work"),
            r"price: \$12\.50"
        );
    }

    #[test]
    fn rejects_duplicate_capture_names() {
        let field = FieldSpec {
            id: "same".to_owned(),
            name: None,
            kind: FieldKind::Integer,
            min_length: 1,
            max_length: None,
            capture: true,
        };
        let spec = PatternSpec::new(
            MatchMode::Full,
            PatternNode::Sequence {
                nodes: vec![
                    PatternNode::Field {
                        field: field.clone(),
                    },
                    PatternNode::Field { field },
                ],
            },
        );

        assert_eq!(
            render(&spec, Dialect::Rust),
            Err(RenderError::DuplicateCaptureName {
                name: "same".to_owned()
            })
        );
    }
}
