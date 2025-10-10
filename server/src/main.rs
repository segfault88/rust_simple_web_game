use axum::{
    Router,
    body::Bytes,
    extract::{
        connect_info::ConnectInfo,
        ws::{CloseFrame, Message, Utf8Bytes, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::{any, get},
};
use axum_extra::TypedHeader;
use futures_util::SinkExt;
use futures_util::StreamExt;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower_http::services::ServeDir;
use tracing::{Level, error, info};
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

    let client_assets_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");

    let app = Router::new()
        .route("/ws", any(ws_handler))
        .route(
            "/count",
            get({
                let handle = handle.clone();
                move || async move {
                    let count = handle.player_count().await;
                    format!("count {}", count)
                }
            }),
        )
        .fallback_service(ServeDir::new(client_assets_path));

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();

    info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown::shutdown_signal(cancel.clone()))
    .await
    .unwrap();

    info!("Server shutdown");
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    info!("agent: `{user_agent}` at {addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr))
}

async fn handle_socket(mut socket: WebSocket, who: SocketAddr) {
    let (mut sender, mut receiver) = socket.split();

    tokio::spawn(async move {
        loop {
            let send = sender.send(Message::Ping(Bytes::from_static(&[]))).await;
            match send {
                Ok(_) => info!("send ping ok"),
                Err(err) => {
                    error!("send ping failed {:?}", err);
                    break;
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });

    tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            info!("got message {:?}", msg)
        }
    });
}
