use std::{env, error::Error, net::SocketAddr};

use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use easyreg_core::{AnalysisResult, AnalyzeRequest};
use easyreg_engine::{AnalysisError, analyze};
use serde::Serialize;

const DEFAULT_ADDRESS: &str = "127.0.0.1:3000";
const MAX_REQUEST_BYTES: usize = 256 * 1024;
const INDEX_HTML: &str = include_str!("../static/index.html");
const APP_CSS: &str = include_str!("../static/app.css");
const APP_JS: &str = include_str!("../static/app.js");

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let address = env::var("EASYREG_ADDR").unwrap_or_else(|_| DEFAULT_ADDRESS.to_owned());
    let address = address.parse::<SocketAddr>()?;
    let listener = tokio::net::TcpListener::bind(address).await?;

    eprintln!("EasyREG web arayüzü http://{address} adresinde çalışıyor");
    axum::serve(listener, app())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn app() -> Router {
    Router::new()
        .route("/", get(index))
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
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn analyze_handler(
    Json(request): Json<AnalyzeRequest>,
) -> Result<Json<AnalysisResult>, ApiError> {
    analyze(&request).map(Json).map_err(ApiError::from)
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
            .candidates
            .iter()
            .find(|candidate| candidate.strategy == result.recommended_strategy)
            .expect("recommended candidate should exist");

        assert_eq!(
            result.recommended_strategy,
            GeneralizationStrategy::Balanced
        );
        assert!(recommended.renderings.contains_key(&Dialect::JavaScript));
        assert!((recommended.validation.negative_rejection - 1.0).abs() < f64::EPSILON);
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
        assert!(APP_CSS.contains(":root"));
        assert!(APP_JS.contains("/api/analyze"));
    }
}
