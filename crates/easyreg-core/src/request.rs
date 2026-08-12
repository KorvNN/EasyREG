use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::MatchMode;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzeRequest {
    pub positive_examples: Vec<String>,
    #[serde(default)]
    pub negative_examples: Vec<String>,
    #[serde(default)]
    pub match_mode: MatchMode,
}

impl AnalyzeRequest {
    /// Checks the minimum invariants required by the inference engine.
    ///
    /// # Errors
    ///
    /// Returns [`InputError`] when no positive example is supplied or one of
    /// the positive examples is empty.
    pub fn validate(&self) -> Result<(), InputError> {
        if self.positive_examples.is_empty() {
            return Err(InputError::NoPositiveExamples);
        }

        if let Some(index) = self.positive_examples.iter().position(String::is_empty) {
            return Err(InputError::EmptyPositiveExample { index });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InputError {
    #[error("at least one positive example is required")]
    NoPositiveExamples,
    #[error("positive example at index {index} is empty")]
    EmptyPositiveExample { index: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_empty_positive_set() {
        let request = AnalyzeRequest {
            positive_examples: Vec::new(),
            negative_examples: Vec::new(),
            match_mode: MatchMode::Full,
        };

        assert_eq!(request.validate(), Err(InputError::NoPositiveExamples));
    }
}
