use std::{collections::BTreeMap, env, error::Error, net::SocketAddr};

use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use easyreg_core::{AnalysisResult, AnalyzeRequest, FieldKind, PatternNode};
use easyreg_engine::{AnalysisError, analyze, apply_field_names};
use easyreg_semantics::{FieldObservation, SemanticField, infer as infer_semantics};
use serde::Serialize;

const DEFAULT_ADDRESS: &str = "127.0.0.1:3000";
const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;
const INDEX_HTML: &str = include_str!("../static/index.html");
const APP_CSS: &str = include_str!("../static/app.css");
const APP_JS: &str = include_str!("../static/app.js");
const ICON_SVG: &str = include_str!("../static/easyreg-icon.svg");

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let address = env::var("EASYREG_ADDR").unwrap_or_else(|_| DEFAULT_ADDRESS.to_owned());
    let address = address.parse::<SocketAddr>()?;
    let listener = tokio::net::TcpListener::bind(address).await?;

    eprintln!("EasyREG web arayüzü http://{address} adresinde çalışıyor");
    eprintln!("Semantik alan isimlendirme tamamen yerel kural motoruyla çalışıyor");
    axum::serve(listener, app())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn app() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/easyreg-icon.svg", get(icon))
        .route("/app.css", get(stylesheet))
        .route("/app.js", get(javascript))
        .route("/api/health", get(health))
        .route("/api/analyze", post(analyze_handler))
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn icon() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")],
        ICON_SVG,
    )
}

async fn stylesheet() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], APP_CSS)
}

async fn javascript() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        APP_JS,
    )
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
    semantics: &'static str,
    external_services: bool,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        semantics: "local_rules",
        external_services: false,
    })
}

async fn analyze_handler(
    Json(request): Json<AnalyzeRequest>,
) -> Result<Json<WebAnalysisResult>, ApiError> {
    let mut analysis = analyze(&request)?;
    let semantics = enrich_semantics(&request, &mut analysis)?;

    Ok(Json(WebAnalysisResult {
        analysis,
        semantics,
    }))
}

#[derive(Debug, Serialize)]
struct WebAnalysisResult {
    #[serde(flatten)]
    analysis: AnalysisResult,
    semantics: SemanticEnrichment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SemanticStatus {
    Complete,
    NotApplicable,
}

#[derive(Debug, Serialize)]
struct SemanticEnrichment {
    status: SemanticStatus,
    engine: &'static str,
    fields: Vec<SemanticField>,
}

fn enrich_semantics(
    request: &AnalyzeRequest,
    analysis: &mut AnalysisResult,
) -> Result<SemanticEnrichment, ApiError> {
    let observations = semantic_observations(analysis);
    if observations.is_empty() {
        return Ok(SemanticEnrichment {
            status: SemanticStatus::NotApplicable,
            engine: "local_rules",
            fields: Vec::new(),
        });
    }

    let fields = infer_semantics(&observations);
    let names = fields
        .iter()
        .map(|field| (field.field_id.clone(), field.name.clone()))
        .collect::<BTreeMap<_, _>>();
    apply_field_names(analysis, request, &names)?;

    Ok(SemanticEnrichment {
        status: SemanticStatus::Complete,
        engine: "local_rules",
        fields,
    })
}

fn semantic_observations(analysis: &AnalysisResult) -> Vec<FieldObservation> {
    let candidate = analysis
        .candidates
        .iter()
        .find(|candidate| candidate.strategy == analysis.recommended_strategy);
    let Some(candidate) = candidate else {
        return Vec::new();
    };

    let mut inferred_fields = Vec::new();
    collect_fields(&candidate.spec.root, "", "", &mut inferred_fields);
    inferred_fields
        .into_iter()
        .map(
            |(field_id, inferred_kind, prefix_literal, suffix_literal)| {
                let samples = candidate
                    .validation
                    .positive_results
                    .iter()
                    .filter_map(|result| result.captures.get(&field_id))
                    .take(8)
                    .map(|value| truncate(value, 160))
                    .collect();
                FieldObservation {
                    field_id,
                    inferred_kind,
                    samples,
                    prefix_literal,
                    suffix_literal,
                }
            },
        )
        .collect()
}

fn collect_fields(
    node: &PatternNode,
    prefix: &str,
    suffix: &str,
    fields: &mut Vec<(String, FieldKind, String, String)>,
) {
    match node {
        PatternNode::Field { field } if field.capture => {
            fields.push((
                field.id.clone(),
                field.kind,
                truncate(prefix, 80),
                truncate(suffix, 80),
            ));
        }
        PatternNode::Sequence { nodes } => {
            for (index, node) in nodes.iter().enumerate() {
                let previous = nodes[..index]
                    .iter()
                    .rev()
                    .find_map(literal_value)
                    .unwrap_or(prefix);
                let next = nodes[index + 1..]
                    .iter()
                    .find_map(literal_value)
                    .unwrap_or(suffix);
                collect_fields(node, previous, next, fields);
            }
        }
        PatternNode::Alternation { branches } => {
            for branch in branches {
                collect_fields(branch, prefix, suffix, fields);
            }
        }
        PatternNode::Repeat { node, .. } => collect_fields(node, prefix, suffix, fields),
        PatternNode::Literal { .. } | PatternNode::Field { .. } => {}
    }
}

fn literal_value(node: &PatternNode) -> Option<&str> {
    match node {
        PatternNode::Literal { value } => Some(value),
        _ => None,
    }
}

fn truncate(value: &str, max_characters: usize) -> String {
    value.chars().take(max_characters).collect()
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl From<AnalysisError> for ApiError {
    fn from(error: AnalysisError) -> Self {
        let (status, code) = match &error {
            AnalysisError::Inference(_) => (StatusCode::BAD_REQUEST, "invalid_request"),
            AnalysisError::Render(_) | AnalysisError::Validation(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "analysis_failed")
            }
        };

        Self {
            status,
            code,
            message: error.to_string(),
        }
    }
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: &self.message,
            },
        };

        (self.status, Json(body)).into_response()
    }
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorEnvelope {
            error: ErrorBody {
                code: "not_found",
                message: "İstenen kaynak bulunamadı.",
            },
        }),
    )
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("Kapatma sinyali dinlenemedi: {error}");
    }
}

#[cfg(test)]
mod tests {
    use easyreg_core::{Dialect, GeneralizationStrategy, MatchMode};

    use super::*;

    #[tokio::test]
    async fn analyzes_a_request_through_the_http_handler() {
        let Json(result) = analyze_handler(Json(AnalyzeRequest {
            positive_examples: vec!["INV-2026-00127".to_owned(), "INV-2025-84621".to_owned()],
            negative_examples: vec!["ORD-2026-00127".to_owned()],
            match_mode: MatchMode::Full,
        }))
        .await
        .expect("HTTP analysis should succeed");
        let recommended = result
            .analysis
            .candidates
            .iter()
            .find(|candidate| candidate.strategy == result.analysis.recommended_strategy)
            .expect("recommended candidate should exist");

        assert_eq!(
            result.analysis.recommended_strategy,
            GeneralizationStrategy::Balanced
        );
        assert!(recommended.renderings.contains_key(&Dialect::JavaScript));
        assert!((recommended.validation.negative_rejection - 1.0).abs() < f64::EPSILON);
        assert_eq!(result.semantics.status, SemanticStatus::Complete);
    }

    #[tokio::test]
    async fn maps_invalid_analysis_input_to_a_bad_request() {
        let error = analyze_handler(Json(AnalyzeRequest {
            positive_examples: Vec::new(),
            negative_examples: Vec::new(),
            match_mode: MatchMode::Full,
        }))
        .await
        .expect_err("empty positive examples should fail");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.code, "invalid_request");
    }

    #[test]
    fn embeds_the_web_application() {
        assert!(INDEX_HTML.contains("EasyREG"));
        assert!(INDEX_HTML.contains("type=\"file\""));
        assert!(INDEX_HTML.contains("id=\"upload-box\""));
        assert!(INDEX_HTML.contains("rel=\"icon\" href=\"/easyreg-icon.svg\""));
        assert!(INDEX_HTML.contains("class=\"brand-mark\" src=\"/easyreg-icon.svg\""));
        assert!(ICON_SVG.contains("<title>EasyREG</title>"));
        assert!(APP_CSS.contains(":root"));
        assert!(APP_JS.contains("/api/analyze"));
        assert!(APP_JS.contains("file.stream().getReader()"));
        assert!(APP_JS.contains("MAX_IMPORTED_LINES"));
        assert!(APP_JS.contains("new TextDecoder(\"utf-8\", { fatal: true })"));
        assert!(APP_JS.contains("matchMode === \"search\" ? \"search\" : \"fullmatch\""));
    }

    #[test]
    fn web_copy_does_not_expose_demo_or_internal_engine_vocabulary() {
        for unwanted in [
            "pattern-orbit",
            "Log parser stüdyosu",
            "Yerel motor hazır",
            "Regex ve parser üretimi",
            "Log biçimini çıkarın",
            "Veri dışarı gönderilmez",
            "<footer>",
            "class=\"hero\"",
            "Motor notları",
            "Pozitif kapsam",
            "Negatif ret",
            "id=\"result-status\"",
        ] {
            assert!(
                !INDEX_HTML.contains(unwanted),
                "web page still exposes {unwanted:?}"
            );
        }
        for unwanted in ["Exact fallback", "Strict", "Balanced", "Flexible"] {
            assert!(
                !APP_JS.contains(unwanted),
                "web script still exposes {unwanted:?}"
            );
        }
    }

    #[test]
    fn builds_bounded_semantic_observations_from_the_recommended_candidate() {
        let request = AnalyzeRequest {
            positive_examples: vec![
                "2026-08-27 14:32:17 ERROR 10.0.0.5 request failed".to_owned(),
                "2026-08-27 14:33:44 WARN 10.0.0.7 request slow".to_owned(),
            ],
            negative_examples: Vec::new(),
            match_mode: MatchMode::Full,
        };
        let analysis = analyze(&request).expect("log analysis should succeed");
        let semantic = semantic_observations(&analysis);

        assert!(
            semantic
                .iter()
                .any(|field| field.inferred_kind == FieldKind::Ipv4)
        );
        assert!(semantic.iter().all(|field| !field.samples.is_empty()));
    }

    #[tokio::test]
    async fn applies_local_semantic_names_without_external_services() {
        let Json(result) = analyze_handler(Json(AnalyzeRequest {
            positive_examples: vec![
                "2026-08-27 14:32:17 ERROR client_ip=10.0.0.5 path=/api/users".to_owned(),
                "2026-08-27 14:33:44 WARN client_ip=10.0.0.7 path=/api/orders".to_owned(),
            ],
            negative_examples: Vec::new(),
            match_mode: MatchMode::Full,
        }))
        .await
        .expect("semantic analysis should succeed");
        let recommended = result
            .analysis
            .candidates
            .iter()
            .find(|candidate| candidate.strategy == result.analysis.recommended_strategy)
            .expect("recommended candidate should exist");

        assert_eq!(result.semantics.status, SemanticStatus::Complete);
        assert!(recommended.renderings[&Dialect::JavaScript].contains("(?<log_date>"));
        assert!(recommended.renderings[&Dialect::JavaScript].contains("(?<source_ip>"));
        assert!(recommended.renderings[&Dialect::JavaScript].contains("(?<path>"));
        assert!((recommended.validation.positive_coverage - 1.0).abs() < f64::EPSILON);
    }
}
