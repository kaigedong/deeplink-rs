use axum::routing::get;
use axum::Router;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

use std::net::SocketAddr;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod handlers;
mod mongo;
mod types;
mod utils;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_websockets=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db = mongo::DB::new().await;
    let app = Router::new()
        .route("/ws", get(handlers::ws_handler).with_state(db))
        // logging so we can see whats going on
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
