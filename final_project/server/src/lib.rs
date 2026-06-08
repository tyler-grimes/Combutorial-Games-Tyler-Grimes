pub mod bot;
pub mod protocol;
pub mod room;
pub mod state;
pub mod ws;

use axum::{routing::any, Router};
use state::SharedState;
use tower_http::services::ServeDir;

pub fn app(state: SharedState) -> Router {
    let static_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/static");
    Router::new()
        .route("/ws", any(ws::ws_handler))
        .fallback_service(ServeDir::new(static_dir))
        .with_state(state)
}
