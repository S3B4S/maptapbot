mod leaderboard;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderName, HeaderValue};
use axum::response::Html;
use axum::routing::get;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::GovernorLayer;
use tower_http::cors::CorsLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tracing::info;
use utoipa::OpenApi;

use crate::db::Database;

pub type DbState = Arc<Mutex<Database>>;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "MapTapBot API",
        description = "Public leaderboard data for MapTap scores.",
        version = "0.1.0"
    ),
    paths(leaderboard::daily),
    components(schemas(leaderboard::LeaderboardEntry)),
    tags((name = "Leaderboard", description = "Score leaderboards"))
)]
struct ApiDoc;

async fn openapi_json() -> axum::Json<utoipa::openapi::OpenApi> {
    axum::Json(ApiDoc::openapi())
}

async fn swagger_ui() -> Html<String> {
    let spec_url = "/api-docs/openapi.json";
    Html(format!(
        r##"<!DOCTYPE html>
<html>
<head>
  <title>MapTapBot API</title>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
</head>
<body>
<div id="swagger-ui"></div>
<script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
<script>
  SwaggerUIBundle({{
    url: "{}",
    dom_id: "#swagger-ui",
    presets: [SwaggerUIBundle.presets.apis, SwaggerUIBundle.SwaggerUIStandalonePreset],
    layout: "BaseLayout"
  }})
</script>
</body>
</html>"##,
        spec_url
    ))
}

pub async fn serve(db: DbState) {
    let port = std::env::var("REST_API_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);

    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(60)
            .burst_size(10)
            .finish()
            .unwrap(),
    );

    // Periodically evict old rate-limit state to avoid unbounded memory growth.
    let limiter = governor_conf.limiter().clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            limiter.retain_recent();
        }
    });

    // `with_state` converts Router<DbState> → Router<()>, making it mergeable
    // with stateless routes.
    let api = Router::new()
        .route("/leaderboard/daily", get(leaderboard::daily))
        .with_state(db);

    let app = Router::new()
        .merge(api)
        .route("/api-docs/openapi.json", get(openapi_json))
        .route("/swagger-ui", get(swagger_ui))
        .layer(GovernorLayer { config: governor_conf })
        .layer(TimeoutLayer::new(Duration::from_secs(10)))
        .layer(CorsLayer::permissive())
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ));

    let addr = format!("0.0.0.0:{port}");
    info!("REST API listening on {addr}");
    info!("OpenAPI spec: http://{addr}/api-docs/openapi.json");
    info!("Swagger UI:   http://{addr}/swagger-ui");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind REST API listener");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("REST API server error");
}
