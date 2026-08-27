//! Local, deterministic naming of fields inferred from log samples.

use std::collections::HashMap;

use easyreg_core::FieldKind;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldObservation {
    pub field_id: String,
    pub inferred_kind: FieldKind,
    pub samples: Vec<String>,
    pub prefix_literal: String,
    pub suffix_literal: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticField {
    pub field_id: String,
    pub name: String,
    pub label: String,
    pub description: String,
    pub confidence: Confidence,
    pub rule: &'static str,
}

/// Assigns portable, unique capture names using only local rules and observed
/// values. No sample data leaves the process.
pub fn infer(observations: &[FieldObservation]) -> Vec<SemanticField> {
    let mut counts = HashMap::<&str, usize>::new();

    observations
        .iter()
        .map(|observation| {
            let classification = classify(observation);
            let count = counts.entry(classification.name).or_default();
            *count += 1;
            let name = if *count == 1 {
                classification.name.to_owned()
            } else {
                format!("{}_{}", classification.name, count)
            };

            SemanticField {
                field_id: observation.field_id.clone(),
                name,
                label: classification.label.to_owned(),
                description: classification.description.to_owned(),
                confidence: classification.confidence,
                rule: classification.rule,
            }
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Classification {
    name: &'static str,
    label: &'static str,
    description: &'static str,
    confidence: Confidence,
    rule: &'static str,
}

fn classify(field: &FieldObservation) -> Classification {
    let context = normalized_context(field);

    classify_context(&context).unwrap_or_else(|| classify_kind(field, &context))
}

fn classify_context(context: &str) -> Option<Classification> {
    if context_has(
        context,
        &["request_id", "request-id", "req_id", "trace_id", "trace-id"],
    ) {
        return Some(classification(
            "request_id",
            "İstek kimliği",
            "İstek veya dağıtık izleme korelasyon kimliği.",
            Confidence::High,
            "context.request_id",
        ));
    }
    if context_has(context, &["user_id", "user-id", "uid"]) {
        return Some(classification(
            "user_id",
            "Kullanıcı kimliği",
            "İşlemi yapan kullanıcıya ait kimlik.",
            Confidence::High,
            "context.user_id",
        ));
    }
    if context_has(context, &["duration", "latency", "elapsed", "took"]) {
        return Some(classification(
            "duration",
            "Süre",
            "İşlemin tamamlanma veya gecikme süresi.",
            Confidence::High,
            "context.duration",
        ));
    }
    if context_has(context, &["status", "status_code", "status-code"]) {
        return Some(classification(
            "http_status",
            "HTTP durumu",
            "HTTP yanıt durum kodu.",
            Confidence::High,
            "context.http_status",
        ));
    }
    if context_has(context, &["port"]) {
        return Some(classification(
            "port",
            "Port",
            "Ağ bağlantısında kullanılan port numarası.",
            Confidence::High,
            "context.port",
        ));
    }

    None
}

fn classify_kind(field: &FieldObservation, context: &str) -> Classification {
    match field.inferred_kind {
        FieldKind::DateIso => classification(
            "log_date",
            "Log tarihi",
            "Olayın ISO takvim tarihi.",
            Confidence::High,
            "kind.date_iso",
        ),
        FieldKind::Time => classification(
            "log_time",
            "Log saati",
            "Olayın 24 saat biçimindeki zamanı.",
            Confidence::High,
            "kind.time",
        ),
        FieldKind::Ipv4 | FieldKind::Ipv6 => classify_ip(context),
        FieldKind::Email => classification(
            "email",
            "E-posta",
            "Log satırında gözlenen e-posta adresi.",
            Confidence::High,
            "kind.email",
        ),
        FieldKind::Url => classification(
            "url",
            "URL",
            "İstek veya kaynak URL'si.",
            Confidence::High,
            "kind.url",
        ),
        FieldKind::Path => classification(
            "path",
            "Yol",
            "İstek veya dosya sistemi yolu.",
            Confidence::High,
            "kind.path",
        ),
        FieldKind::Uuid => classification(
            "uuid",
            "UUID",
            "Log satırında gözlenen UUID değeri.",
            Confidence::Medium,
            "kind.uuid",
        ),
        FieldKind::Uppercase | FieldKind::Lowercase | FieldKind::Alphabetic => {
            classify_words(field)
        }
        FieldKind::Integer => classify_integer(field, context),
        FieldKind::Decimal => classification(
            "decimal_value",
            "Ondalık değer",
            "Bağlamı henüz belirlenmemiş ondalık alan.",
            Confidence::Low,
            "kind.decimal",
        ),
        FieldKind::Hexadecimal => classification(
            "hex_value",
            "Hexadecimal değer",
            "Hexadecimal biçimindeki tanımlayıcı veya sayısal alan.",
            Confidence::Medium,
            "kind.hexadecimal",
        ),
        FieldKind::Alphanumeric => classification(
            "identifier",
            "Tanımlayıcı",
            "Harf ve rakamlardan oluşan tanımlayıcı.",
            Confidence::Low,
            "kind.alphanumeric",
        ),
        FieldKind::NonWhitespace | FieldKind::Text => classification(
            "value",
            "Değer",
            "Bağlamı güvenle belirlenemeyen değişken alan.",
            Confidence::Low,
            "fallback.value",
        ),
        FieldKind::Whitespace => classification(
            "whitespace",
            "Boşluk",
            "Yapısal boşluk alanı.",
            Confidence::High,
            "kind.whitespace",
        ),
    }
}

fn classify_ip(context: &str) -> Classification {
    if context_has(
        context,
        &[
            "src",
            "source",
            "source_ip",
            "client",
            "client_ip",
            "remote",
            "remote_ip",
            "remote_addr",
        ],
    ) {
        classification(
            "source_ip",
            "Kaynak IP",
            "İsteği veya bağlantıyı başlatan IP adresi.",
            Confidence::High,
            "context.source_ip",
        )
    } else if context_has(
        context,
        &[
            "dst",
            "destination",
            "destination_ip",
            "server",
            "server_ip",
            "upstream",
            "upstream_addr",
        ],
    ) {
        classification(
            "destination_ip",
            "Hedef IP",
            "İsteğin veya bağlantının hedef IP adresi.",
            Confidence::High,
            "context.destination_ip",
        )
    } else {
        classification(
            "ip_address",
            "IP adresi",
            "Kaynak veya hedef rolü belirtilmemiş IP adresi.",
            Confidence::Medium,
            "kind.ip",
        )
    }
}

fn classify_words(field: &FieldObservation) -> Classification {
    let values = normalized_values(field);
    if !values.is_empty() && values.iter().all(|value| is_log_level(value)) {
        return classification(
            "level",
            "Log seviyesi",
            "Olayın önem veya hata seviyesi.",
            Confidence::High,
            "values.log_level",
        );
    }
    if !values.is_empty() && values.iter().all(|value| is_http_method(value)) {
        return classification(
            "http_method",
            "HTTP metodu",
            "İstekte kullanılan HTTP metodu.",
            Confidence::High,
            "values.http_method",
        );
    }

    classification(
        "text_value",
        "Metin değeri",
        "Bağlamı henüz belirlenmemiş metinsel alan.",
        Confidence::Low,
        "kind.textual",
    )
}

fn classify_integer(field: &FieldObservation, context: &str) -> Classification {
    let integers = field
        .samples
        .iter()
        .filter_map(|value| value.parse::<u64>().ok())
        .collect::<Vec<_>>();
    if !integers.is_empty()
        && integers.iter().all(|value| (100..=599).contains(value))
        && context_has(context, &["http", "response", "request"])
    {
        return classification(
            "http_status",
            "HTTP durumu",
            "HTTP yanıt durum kodu.",
            Confidence::Medium,
            "values.http_status_range",
        );
    }

    classification(
        "number",
        "Sayısal değer",
        "Bağlamı henüz belirlenmemiş tam sayı alanı.",
        Confidence::Low,
        "kind.integer",
    )
}

const fn classification(
    name: &'static str,
    label: &'static str,
    description: &'static str,
    confidence: Confidence,
    rule: &'static str,
) -> Classification {
    Classification {
        name,
        label,
        description,
        confidence,
        rule,
    }
}

fn normalized_context(field: &FieldObservation) -> String {
    field.prefix_literal.to_ascii_lowercase().replace('-', "_")
}

fn normalized_values(field: &FieldObservation) -> Vec<String> {
    field
        .samples
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn context_has(context: &str, needles: &[&str]) -> bool {
    context
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|word| !word.is_empty())
        .any(|word| {
            needles.iter().any(|needle| {
                let normalized_needle = needle.replace('-', "_");
                word == normalized_needle
                    || (!normalized_needle.contains('_')
                        && word.split('_').any(|part| part == normalized_needle))
            })
        })
}

fn is_log_level(value: &str) -> bool {
    matches!(
        value,
        "trace"
            | "debug"
            | "info"
            | "notice"
            | "warn"
            | "warning"
            | "error"
            | "fatal"
            | "critical"
            | "crit"
            | "alert"
            | "emerg"
    )
}

fn is_http_method(value: &str) -> bool {
    matches!(
        value,
        "get" | "head" | "post" | "put" | "patch" | "delete" | "options" | "connect" | "trace"
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    struct SemanticRuleCase {
        kind: FieldKind,
        samples: &'static [&'static str],
        prefix: &'static str,
        expected_name: &'static str,
        expected_rule: &'static str,
        expected_confidence: Confidence,
    }

    const SEMANTIC_RULE_CASES: &[SemanticRuleCase] = &[
        SemanticRuleCase {
            kind: FieldKind::Uuid,
            samples: &["550e8400-e29b-41d4-a716-446655440000"],
            prefix: " request_id=",
            expected_name: "request_id",
            expected_rule: "context.request_id",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Integer,
            samples: &["42"],
            prefix: " user_id=",
            expected_name: "user_id",
            expected_rule: "context.user_id",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Decimal,
            samples: &["18.4"],
            prefix: " latency=",
            expected_name: "duration",
            expected_rule: "context.duration",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Integer,
            samples: &["200", "404"],
            prefix: " status_code=",
            expected_name: "http_status",
            expected_rule: "context.http_status",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Integer,
            samples: &["443"],
            prefix: " port=",
            expected_name: "port",
            expected_rule: "context.port",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::DateIso,
            samples: &["2026-08-27"],
            prefix: " ",
            expected_name: "log_date",
            expected_rule: "kind.date_iso",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Time,
            samples: &["14:32:17.120"],
            prefix: " ",
            expected_name: "log_time",
            expected_rule: "kind.time",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Ipv4,
            samples: &["10.0.0.5"],
            prefix: " client_ip=",
            expected_name: "source_ip",
            expected_rule: "context.source_ip",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Ipv4,
            samples: &["10.0.0.9"],
            prefix: " destination_ip=",
            expected_name: "destination_ip",
            expected_rule: "context.destination_ip",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Ipv6,
            samples: &["2001:db8::1"],
            prefix: " ",
            expected_name: "ip_address",
            expected_rule: "kind.ip",
            expected_confidence: Confidence::Medium,
        },
        SemanticRuleCase {
            kind: FieldKind::Email,
            samples: &["ops@example.com"],
            prefix: " ",
            expected_name: "email",
            expected_rule: "kind.email",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Url,
            samples: &["https://example.com/api"],
            prefix: " ",
            expected_name: "url",
            expected_rule: "kind.url",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Path,
            samples: &["/api/users"],
            prefix: " ",
            expected_name: "path",
            expected_rule: "kind.path",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Uuid,
            samples: &["550e8400-e29b-41d4-a716-446655440000"],
            prefix: " ",
            expected_name: "uuid",
            expected_rule: "kind.uuid",
            expected_confidence: Confidence::Medium,
        },
        SemanticRuleCase {
            kind: FieldKind::Uppercase,
            samples: &["INFO", "ERROR"],
            prefix: " ",
            expected_name: "level",
            expected_rule: "values.log_level",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Uppercase,
            samples: &["GET", "POST"],
            prefix: " method=",
            expected_name: "http_method",
            expected_rule: "values.http_method",
            expected_confidence: Confidence::High,
        },
        SemanticRuleCase {
            kind: FieldKind::Integer,
            samples: &["201", "503"],
            prefix: " http response=",
            expected_name: "http_status",
            expected_rule: "values.http_status_range",
            expected_confidence: Confidence::Medium,
        },
        SemanticRuleCase {
            kind: FieldKind::Integer,
            samples: &["7", "42"],
            prefix: " ",
            expected_name: "number",
            expected_rule: "kind.integer",
            expected_confidence: Confidence::Low,
        },
        SemanticRuleCase {
            kind: FieldKind::Decimal,
            samples: &["1.25"],
            prefix: " ",
            expected_name: "decimal_value",
            expected_rule: "kind.decimal",
            expected_confidence: Confidence::Low,
        },
        SemanticRuleCase {
            kind: FieldKind::Hexadecimal,
            samples: &["0xCAFE"],
            prefix: " ",
            expected_name: "hex_value",
            expected_rule: "kind.hexadecimal",
            expected_confidence: Confidence::Medium,
        },
        SemanticRuleCase {
            kind: FieldKind::Alphanumeric,
            samples: &["Node42"],
            prefix: " ",
            expected_name: "identifier",
            expected_rule: "kind.alphanumeric",
            expected_confidence: Confidence::Low,
        },
        SemanticRuleCase {
            kind: FieldKind::Alphabetic,
            samples: &["Ready", "Running"],
            prefix: " ",
            expected_name: "text_value",
            expected_rule: "kind.textual",
            expected_confidence: Confidence::Low,
        },
        SemanticRuleCase {
            kind: FieldKind::NonWhitespace,
            samples: &["node_42"],
            prefix: " ",
            expected_name: "value",
            expected_rule: "fallback.value",
            expected_confidence: Confidence::Low,
        },
        SemanticRuleCase {
            kind: FieldKind::Whitespace,
            samples: &["  "],
            prefix: "",
            expected_name: "whitespace",
            expected_rule: "kind.whitespace",
            expected_confidence: Confidence::High,
        },
    ];

    fn field(kind: FieldKind, samples: &[&str], prefix: &str) -> FieldObservation {
        FieldObservation {
            field_id: "field_1".to_owned(),
            inferred_kind: kind,
            samples: samples.iter().map(|value| (*value).to_owned()).collect(),
            prefix_literal: prefix.to_owned(),
            suffix_literal: String::new(),
        }
    }

    #[test]
    fn every_semantic_rule_has_a_canonical_observation() {
        let mut rules = BTreeSet::new();

        for case in SEMANTIC_RULE_CASES {
            let classification = classify(&field(case.kind, case.samples, case.prefix));
            assert_eq!(
                classification.name, case.expected_name,
                "{} returned the wrong name",
                case.expected_rule
            );
            assert_eq!(classification.rule, case.expected_rule);
            assert_eq!(classification.confidence, case.expected_confidence);
            assert!(
                rules.insert(classification.rule),
                "semantic rule {} has duplicate canonical cases",
                classification.rule
            );
        }

        assert_eq!(rules.len(), SEMANTIC_RULE_CASES.len());
    }

    #[test]
    fn names_common_log_fields_without_a_provider() {
        let fields = infer(&[
            field(FieldKind::Uppercase, &["ERROR", "WARN"], " "),
            field(FieldKind::Ipv4, &["10.0.0.5"], " client_ip="),
            field(FieldKind::Path, &["/api/users"], " path="),
        ]);

        assert_eq!(fields[0].name, "level");
        assert_eq!(fields[1].name, "source_ip");
        assert_eq!(fields[2].name, "path");
        assert!(
            fields
                .iter()
                .all(|field| field.confidence == Confidence::High)
        );
    }

    #[test]
    fn uses_context_to_disambiguate_integers() {
        let fields = infer(&[
            field(FieldKind::Integer, &["200", "404"], " status="),
            field(FieldKind::Integer, &["42", "81"], " duration="),
        ]);

        assert_eq!(fields[0].name, "http_status");
        assert_eq!(fields[1].name, "duration");
    }

    #[test]
    fn does_not_leak_the_following_field_context_to_the_current_field() {
        let fields = infer(&[
            FieldObservation {
                field_id: "field_1".to_owned(),
                inferred_kind: FieldKind::Path,
                samples: vec!["/api/users".to_owned()],
                prefix_literal: " path=".to_owned(),
                suffix_literal: " status=".to_owned(),
            },
            FieldObservation {
                field_id: "field_2".to_owned(),
                inferred_kind: FieldKind::Integer,
                samples: vec!["500".to_owned()],
                prefix_literal: " status=".to_owned(),
                suffix_literal: " duration=".to_owned(),
            },
        ]);

        assert_eq!(fields[0].name, "path");
        assert_eq!(fields[1].name, "http_status");
    }

    #[test]
    fn makes_repeated_names_portable_and_unique() {
        let fields = infer(&[
            field(FieldKind::Ipv4, &["10.0.0.5"], " "),
            field(FieldKind::Ipv4, &["10.0.0.7"], " "),
        ]);

        assert_eq!(fields[0].name, "ip_address");
        assert_eq!(fields[1].name, "ip_address_2");
    }
}
