use axum::{Router, routing::get};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

mod game;
mod shutdown;

#[tokio::main]
async fn main() {
    let cancel = CancellationToken::new();

    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::DEBUG)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let port = std::env::var("PORT").unwrap_or_else(|_| "8888".to_string());

    let (game, handle) = game::Game::new();

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
                    format!("Hello, World! player id: {}", player_id)
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

    info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown::shutdown_signal(cancel.clone()))
        .await
        .unwrap();

    info!("Server shutdown");
}
