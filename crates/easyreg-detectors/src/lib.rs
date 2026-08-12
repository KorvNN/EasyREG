//! Deterministic value detectors used before any semantic model is consulted.

use std::net::{Ipv4Addr, Ipv6Addr};

use easyreg_core::FieldKind;

pub trait Detector: Send + Sync {
    fn kind(&self) -> FieldKind;
    fn matches(&self, value: &str) -> bool;
}

pub struct DetectorRegistry {
    detectors: Vec<Box<dyn Detector>>,
}

impl Default for DetectorRegistry {
    fn default() -> Self {
        Self {
            detectors: vec![
                Box::new(Ipv4Detector),
                Box::new(Ipv6Detector),
                Box::new(UuidDetector),
                Box::new(EmailDetector),
                Box::new(UrlDetector),
                Box::new(DateIsoDetector),
                Box::new(HexDetector),
                Box::new(DecimalDetector),
                Box::new(IntegerDetector),
                Box::new(UppercaseDetector),
                Box::new(LowercaseDetector),
                Box::new(AlphabeticDetector),
                Box::new(AlphanumericDetector),
                Box::new(WhitespaceDetector),
                Box::new(NonWhitespaceDetector),
            ],
        }
    }
}

impl DetectorRegistry {
    /// Returns the most specific detector that accepts every observed value.
    pub fn classify_all(&self, values: &[&str]) -> FieldKind {
        if values.is_empty() {
            return FieldKind::Text;
        }

        self.detectors
            .iter()
            .find(|detector| values.iter().all(|value| detector.matches(value)))
            .map_or(FieldKind::Text, |detector| detector.kind())
    }
}

struct Ipv4Detector;

impl Detector for Ipv4Detector {
    fn kind(&self) -> FieldKind {
        FieldKind::Ipv4
    }

    fn matches(&self, value: &str) -> bool {
        value.parse::<Ipv4Addr>().is_ok()
    }
}

struct Ipv6Detector;

impl Detector for Ipv6Detector {
    fn kind(&self) -> FieldKind {
        FieldKind::Ipv6
    }

    fn matches(&self, value: &str) -> bool {
        value.contains(':') && value.parse::<Ipv6Addr>().is_ok()
    }
}

struct UuidDetector;

impl Detector for UuidDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::Uuid
    }

    fn matches(&self, value: &str) -> bool {
        const GROUP_LENGTHS: [usize; 5] = [8, 4, 4, 4, 12];
        let groups = value.split('-').collect::<Vec<_>>();

        groups.len() == GROUP_LENGTHS.len()
            && groups
                .iter()
                .zip(GROUP_LENGTHS)
                .all(|(group, expected)| {
                    group.len() == expected && group.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
    }
}

struct EmailDetector;

impl Detector for EmailDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::Email
    }

    fn matches(&self, value: &str) -> bool {
        if value.chars().any(char::is_whitespace) {
            return false;
        }

        let mut parts = value.split('@');
        let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
            return false;
        };

        !local.is_empty()
            && domain.contains('.')
            && domain
                .split('.')
                .all(|label| !label.is_empty() && !label.starts_with('-') && !label.ends_with('-'))
    }
}

struct UrlDetector;

impl Detector for UrlDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::Url
    }

    fn matches(&self, value: &str) -> bool {
        if value.chars().any(char::is_whitespace) {
            return false;
        }

        let authority = value
            .strip_prefix("https://")
            .or_else(|| value.strip_prefix("http://"))
            .and_then(|remainder| remainder.split('/').next());

        authority.is_some_and(|host| !host.is_empty())
    }
}

struct DateIsoDetector;

impl Detector for DateIsoDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::DateIso
    }

    fn matches(&self, value: &str) -> bool {
        let bytes = value.as_bytes();
        if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return false;
        }

        if !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
        {
            return false;
        }

        let Ok(year) = value[0..4].parse::<u32>() else {
            return false;
        };
        let Ok(month) = value[5..7].parse::<u32>() else {
            return false;
        };
        let Ok(day) = value[8..10].parse::<u32>() else {
            return false;
        };

        let max_day = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if is_leap_year(year) => 29,
            2 => 28,
            _ => return false,
        };

        (1..=max_day).contains(&day)
    }
}

const fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

struct HexDetector;

impl Detector for HexDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::Hexadecimal
    }

    fn matches(&self, value: &str) -> bool {
        value
            .strip_prefix("0x")
            .or_else(|| value.strip_prefix("0X"))
            .is_some_and(|digits| {
                !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
    }
}

struct DecimalDetector;

impl Detector for DecimalDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::Decimal
    }

    fn matches(&self, value: &str) -> bool {
        let mut parts = value.split('.');
        let (Some(integer), Some(fraction), None) = (parts.next(), parts.next(), parts.next()) else {
            return false;
        };

        !integer.is_empty()
            && !fraction.is_empty()
            && integer.bytes().all(|byte| byte.is_ascii_digit())
            && fraction.bytes().all(|byte| byte.is_ascii_digit())
    }
}

struct IntegerDetector;

impl Detector for IntegerDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::Integer
    }

    fn matches(&self, value: &str) -> bool {
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
    }
}

struct UppercaseDetector;

impl Detector for UppercaseDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::Uppercase
    }

    fn matches(&self, value: &str) -> bool {
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_uppercase())
    }
}

struct LowercaseDetector;

impl Detector for LowercaseDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::Lowercase
    }

    fn matches(&self, value: &str) -> bool {
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_lowercase())
    }
}

struct AlphabeticDetector;

impl Detector for AlphabeticDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::Alphabetic
    }

    fn matches(&self, value: &str) -> bool {
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphabetic())
    }
}

struct AlphanumericDetector;

impl Detector for AlphanumericDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::Alphanumeric
    }

    fn matches(&self, value: &str) -> bool {
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }
}

struct WhitespaceDetector;

impl Detector for WhitespaceDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::Whitespace
    }

    fn matches(&self, value: &str) -> bool {
        !value.is_empty() && value.chars().all(char::is_whitespace)
    }
}

struct NonWhitespaceDetector;

impl Detector for NonWhitespaceDetector {
    fn kind(&self) -> FieldKind {
        FieldKind::NonWhitespace
    }

    fn matches(&self, value: &str) -> bool {
        !value.is_empty() && value.chars().all(|character| !character.is_whitespace())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chooses_semantic_detectors_before_lexical_detectors() {
        let registry = DetectorRegistry::default();

        assert_eq!(
            registry.classify_all(&["192.168.1.1", "10.0.0.7"]),
            FieldKind::Ipv4
        );
        assert_eq!(
            registry.classify_all(&[
                "550e8400-e29b-41d4-a716-446655440000",
                "123e4567-e89b-12d3-a456-426614174000",
            ]),
            FieldKind::Uuid
        );
    }

    #[test]
    fn validates_calendar_dates() {
        let registry = DetectorRegistry::default();

        assert_eq!(
            registry.classify_all(&["2024-02-29", "2026-08-12"]),
            FieldKind::DateIso
        );
        assert_ne!(
            registry.classify_all(&["2023-02-29", "2026-08-12"]),
            FieldKind::DateIso
        );
    }

    #[test]
    fn falls_back_to_non_whitespace_for_unknown_tokens() {
        let registry = DetectorRegistry::default();

        assert_eq!(
            registry.classify_all(&["abc_123", "xyz_987"]),
            FieldKind::NonWhitespace
        );
    }
}
