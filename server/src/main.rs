use axum::extract::ws::{CloseFrame as axCloseFrame, Message as axMessage};
use axum::{
    Router,
    body::Bytes,
    extract::{
        connect_info::ConnectInfo,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::{any, get},
};
use axum_extra::TypedHeader;
use futures_util::SinkExt;
use futures_util::StreamExt;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tokio::sync::mpsc::unbounded_channel;
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

    let client_assets_path: PathBuf = if let Ok(path) = std::env::var("ASSETS_PATH") {
        // 1. Production/Configured: Use the path from the environment variable
        PathBuf::from(path)
    } else {
        // 2. Local Development Fallback: Assume the 'static' directory is in the crate root
        // This uses the compile-time path, which only works during 'cargo run'
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static")
    };

    info!("set assets path: {:?}", client_assets_path);

    let app = Router::new()
        .route(
            "/ws",
            any({
                let handle = handle.clone();
                move |ws, user_agent, addr| ws_handler(ws, user_agent, addr, handle.clone())
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
    handle: std::sync::Arc<game::GameHandle>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    info!("agent: `{user_agent}` at {addr} starting upgrade");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, addr, handle))
}

async fn handle_socket(
    mut socket: WebSocket,
    who: SocketAddr,
    handle: std::sync::Arc<game::GameHandle>,
) {
    let (ws_message_send, mut ws_message_rec) = unbounded_channel::<game::WsMessage>();
    // Register player with the game
    let player_id = handle.add_player(ws_message_send).await;

    // Helper macro for sending messages with consistent error handling - Claude suggested this
    macro_rules! send_or_return {
        ($msg:expr) => {
            if let Err(e) = socket.send($msg).await {
                info!("player {} send failed: {:?}, returning", player_id, e);
                return;
            }
        };
    }

    {
        let joined = shared::ClientWsMessage::Joined(player_id);
        // let mut buffer: Vec<u8> = Vec::new();

        // if let Err(error) =
        //     bincode::encode_into_slice(joined, &mut buffer, bincode::config::standard())
        // {
        //     error!("encode_into_slice failed, error: {:?}", error);
        //     return;
        // };
        match bincode::encode_to_vec(joined, bincode::config::standard()) {
            Ok(msg_bytes) => {
                send_or_return!(axMessage::Binary(msg_bytes.into()))
            }
            Err(error) => {
                error!("encode_to_vec error {:?}, returning", error);
                return;
            }
        }
    }

    info!("player {} joined from {}", player_id, who);

    loop {
        tokio::select! {
            from_game = ws_message_rec.recv() => {
                match from_game {
                    None => {
                        info!("recv none from game, returning for player_id {:?}", player_id);
                        return
                     },
                    Some(game::WsMessage::Kick)=>{
                        info!("kicking player {}", player_id);
                        send_or_return!(
                            axMessage::Close(Some(axCloseFrame{code:std::u16::MAX, reason: "kicked".into()}))
                        )
                    }
                }
            },
            from_player = socket.recv() => {
                match from_player {
                    None => {
                        // Connection closed gracefully
                        info!("player {} connection closed", player_id);
                        return
                    },
                    Some(Ok(message)) => {
                        // Handle the websocket message from the player
                        info!("player {} sent message: {:?}", player_id, message);
                    },
                    Some(Err(error)) => {
                        // Connection error
                        info!("player {} connection error: {:?}, returning", player_id, error);
                        return
                    }
                }
            }
        }
    }
}
