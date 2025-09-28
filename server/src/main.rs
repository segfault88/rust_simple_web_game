use axum::{Router, routing::get};
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

    let (game, handle) = Game::new();

    tokio::spawn({
        let cancel = cancel.clone();
        async move {
            game.start(cancel).await;
        }
    });

    let app = Router::new()
        .route(
            "/",
            get({
                let handle = handle.clone();
                move || async move {
                    let player_id = handle.add_player().await;
                    format!(
                        "Hello, World! player id: {}, total players: {}",
                        player_id,
                        handle.player_count().await
                    )
                }
            }),
        )
        .route(
            "/count",
            get({
                let handle = handle.clone();
                move || async move {
                    let count = handle.player_count().await;
                    format!("count {}", count)
                }
            }),
        );

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
