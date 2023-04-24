use axum::routing::get;
use axum::Router;
use mongo::init_mongo;

use std::net::SocketAddr;

mod handlers;
mod mongo;
mod types;

#[tokio::main]
async fn main() {
    let client = init_mongo().await.unwrap();
    let app = Router::new().route("/ws", get(handlers::ws_handler).with_state(client));
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));

    if let Err(e) = mongo::init_mongo().await {
        println!("Init mongo failed: {:?}", e)
    }

    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
