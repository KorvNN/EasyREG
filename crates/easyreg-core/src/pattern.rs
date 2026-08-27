use serde::{Deserialize, Serialize};

/// Determines whether the generated expression consumes the complete input or
/// searches for a matching fragment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    #[default]
    Full,
    Search,
}

/// The amount of generalization applied to observed examples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneralizationStrategy {
    Strict,
    Balanced,
    Flexible,
}

/// A target regular-expression dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dialect {
    JavaScript,
    Python,
    Pcre2,
    /// Internal safe evaluator based on Rust's linear-time regex engine.
    Rust,
}

/// A semantic or lexical classification for a variable field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Ipv4,
    Ipv6,
    Uuid,
    Email,
    Url,
    Path,
    DateIso,
    Time,
    Integer,
    Decimal,
    Hexadecimal,
    Uppercase,
    Lowercase,
    Alphabetic,
    Alphanumeric,
    Whitespace,
    NonWhitespace,
    Text,
}

impl FieldKind {
    pub const fn has_intrinsic_shape(self) -> bool {
        matches!(
            self,
            Self::Ipv4
                | Self::Ipv6
                | Self::Uuid
                | Self::Email
                | Self::Url
                | Self::Path
                | Self::DateIso
                | Self::Time
        )
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ipv4 => "ipv4",
            Self::Ipv6 => "ipv6",
            Self::Uuid => "uuid",
            Self::Email => "email",
            Self::Url => "url",
            Self::Path => "path",
            Self::DateIso => "date_iso",
            Self::Time => "time",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
            Self::Hexadecimal => "hexadecimal",
            Self::Uppercase => "uppercase",
            Self::Lowercase => "lowercase",
            Self::Alphabetic => "alphabetic",
            Self::Alphanumeric => "alphanumeric",
            Self::Whitespace => "whitespace",
            Self::NonWhitespace => "non_whitespace",
            Self::Text => "text",
        }
    }
}

/// Description of a captured (or structural) variable in a pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldSpec {
    pub id: String,
    pub name: Option<String>,
    pub kind: FieldKind,
    pub min_length: usize,
    pub max_length: Option<usize>,
    pub capture: bool,
}

impl FieldSpec {
    pub fn capture_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

/// Engine-neutral representation from which target regex strings are rendered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatternNode {
    Literal {
        value: String,
    },
    Field {
        field: FieldSpec,
    },
    Sequence {
        nodes: Vec<Self>,
    },
    Alternation {
        branches: Vec<Self>,
    },
    Repeat {
        node: Box<Self>,
        min: usize,
        max: Option<usize>,
    },
}

impl PatternNode {
    pub fn node_count(&self) -> usize {
        match self {
            Self::Literal { .. } | Self::Field { .. } => 1,
            Self::Sequence { nodes } => 1 + nodes.iter().map(Self::node_count).sum::<usize>(),
            Self::Alternation { branches } => {
                1 + branches.iter().map(Self::node_count).sum::<usize>()
            }
            Self::Repeat { node, .. } => 1 + node.node_count(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternSpec {
    pub schema_version: u16,
    pub match_mode: MatchMode,
    pub root: PatternNode,
}

impl PatternSpec {
    pub const CURRENT_SCHEMA_VERSION: u16 = 1;

    pub fn new(match_mode: MatchMode, root: PatternNode) -> Self {
        Self {
            schema_version: Self::CURRENT_SCHEMA_VERSION,
            match_mode,
            root,
        }
    }
}
