mod api;
mod db;
mod models;
mod utils;
mod ws;

use axum::{
    Router,
    http::{
        HeaderValue, Method,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    routing::get,
};
use dotenv::dotenv;
use std::sync::Arc;
use std::time::Duration;
use tower_cookies::CookieManagerLayer;
use tower_http::cors::CorsLayer;
// use tokio::sync::broadcast;
use std::collections::HashMap;
use tokio::sync::Mutex;

use crate::{
    api::{auth::auth_router, room::room_router},
    db::connection::Database,
    ws::AppState,
};

#[derive(Clone)]
pub struct SharedState {
    pub db: Arc<Database>,
    pub ws_state: Arc<AppState>,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let db = Arc::new(
        Database::init()
            .await
            .expect("❌ Failed to connect to MongoDB"),
    );
    // let (tx, _rx) = broadcast::channel(100);
    let user_sockets = Arc::new(Mutex::new(HashMap::new()));
    let sockets = Arc::new(Mutex::new(HashMap::new()));

    let app_state = Arc::new(AppState {
        user_sockets,
        sockets,
    });

    let shared_state = SharedState {
        db: db.clone(),
        ws_state: app_state,
    };

    let cors = CorsLayer::new()
        .allow_origin(HeaderValue::from_static("http://localhost:5173"))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION])
        .allow_credentials(true)
        .max_age(Duration::from_secs(3600));

    let app = Router::new()
        .nest("/auth", auth_router())
        .nest("/room", room_router())
        .route("/ws", get(ws::handler))
        .layer(CookieManagerLayer::new())
        .layer(cors)
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
