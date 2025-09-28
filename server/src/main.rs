use axum::{routing::get, Router};
use std::sync::Arc;
use tokio::{net::TcpListener, signal};
use tokio_util::sync::CancellationToken;
// use crate::game::Game;

mod game;
use game::Game;

async fn shutdown_signal(cancel: CancellationToken) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    tokio::select! {
        _ = ctrl_c => {
            println!("Ctrl+C received, cancelling...");
            cancel.cancel();
        },
        _ = cancel.cancelled() => {},
    }
}

#[tokio::main]
async fn main() {
    let cancel = CancellationToken::new();

    let port = std::env::var("PORT").unwrap_or_else(|_| "8888".to_string());

    let game = Arc::new(Game::new());
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        game.clone().run(cancel_clone).await;
    });

    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    println!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(cancel.clone()))
        .await
        .unwrap();

    println!("Server shutdown");
}
