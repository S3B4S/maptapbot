mod leaderboard;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderName, HeaderValue};
use axum::routing::get;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::GovernorLayer;
use tower_http::cors::CorsLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::timeout::TimeoutLayer;
use tracing::info;

use crate::db::Database;

pub type DbState = Arc<Mutex<Database>>;

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

    let app = Router::new()
        .route("/leaderboard/daily", get(leaderboard::daily))
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
        ))
        .with_state(db);

    let addr = format!("0.0.0.0:{port}");
    info!("REST API listening on {addr}");
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
