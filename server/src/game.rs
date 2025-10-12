use shared::OtherPlayer;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::time::{Duration, interval};
use tokio_util::sync::CancellationToken;
use tracing::info;

const TICKS_PER_SECOND: u64 = 1;

pub enum WsMessage {
    _Kick,
    Spawn(shared::Position, Vec<OtherPlayer>),
    PlayerSpawn(OtherPlayer),
}

#[derive(Debug)]
struct Player {
    player_id: shared::PlayerId,
    state: shared::PlayerState,
    position: shared::Position,
    ws_handler: UnboundedSender<WsMessage>,
}

impl Player {
    pub fn to_other_player(&self) -> OtherPlayer {
        OtherPlayer {
            player_id: self.player_id,
            position: self.position.clone(),
        }
    }
}

#[derive(Debug)]
pub struct GameState {
    player_count: shared::PlayerId,
    next_player_id: shared::PlayerId,
    send: UnboundedSender<GameAction>,
    receive: UnboundedReceiver<GameAction>,
    players: HashMap<shared::PlayerId, Player>,
}

impl GameState {
    pub fn new() -> Self {
        let (send, receive) = unbounded_channel::<GameAction>();

        GameState {
            player_count: 0,
            next_player_id: 1,
            send,
            receive,
            players: HashMap::new(),
        }
    }
}

enum GameAction {
    Join {
        player_id: shared::PlayerId,
        ws_sender: UnboundedSender<WsMessage>,
    },
    Leave {
        player_id: shared::PlayerId,
    },
}

pub struct Game {
    state: Arc<RwLock<GameState>>,
}

impl Game {
    pub fn new() -> (Self, Arc<GameHandle>) {
        let state = Arc::new(RwLock::new(GameState::new()));

        let game = Game {
            state: state.clone(),
        };

        let handle = GameHandle {
            state: state.clone(),
        };

        (game, Arc::new(handle))
    }

    pub async fn start(&self, cancel_token: CancellationToken) {
        info!("Game loop starting");

        let tick_interval = Duration::from_micros((1e6 as u64) / TICKS_PER_SECOND);
        let mut tick_timer = interval(tick_interval);

        // Skip the first tick which fires immediately
        tick_timer.tick().await;

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("Game loop canceled");
                    break
                }
                _ = tick_timer.tick() => {
                    self.game_tick().await;
                }
            }
        }
    }

    pub async fn game_tick(&self) {
        let mut lock = self.state.write().await;

        while let Ok(action) = lock.receive.try_recv() {
            match action {
                GameAction::Join {
                    player_id,
                    ws_sender,
                } => {
                    info!(player_id, "completing join");
                    lock.players.insert(
                        player_id,
                        Player {
                            player_id: player_id,
                            state: shared::PlayerState::BeforeSpawn,
                            position: shared::Position::new(player_id),
                            ws_handler: ws_sender,
                        },
                    );
                }
                GameAction::Leave { player_id } => {
                    info!(player_id, "completing leave");
                    lock.players.remove(&player_id);
                    lock.player_count = lock.player_count.saturating_sub(1);
                }
            }
        }

        // collect players that need to be spawned
        let players_to_spawn: Vec<(shared::PlayerId, shared::Position)> = lock
            .players
            .iter()
            .filter(|(_, p)| matches!(p.state, shared::PlayerState::BeforeSpawn))
            .map(|(&id, player)| (id, player.position.clone()))
            .collect();

        // spawn each player
        for (player_id, spawn_at) in players_to_spawn {
            // collect other players (all except the one being spawned)
            let other_players: Vec<OtherPlayer> = lock
                .players
                .values()
                .filter(|p| p.player_id != player_id)
                .map(|p| p.to_other_player())
                .collect();

            // now mutate the current player
            if let Some(player) = lock.players.get_mut(&player_id) {
                info!("spawning player id: {:?} at: {:?}", player_id, spawn_at);
                player.state = shared::PlayerState::Alive;

                player
                    .ws_handler
                    .send(WsMessage::Spawn(spawn_at.clone(), other_players.clone()))
                    .unwrap(); // todo: remove unwrap
            }

            // now notify other plays of the spawn
            for other_player in other_players {
                if let Some(player) = lock.players.get(&other_player.player_id) {
                    player
                        .ws_handler
                        .send(WsMessage::PlayerSpawn(OtherPlayer {
                            player_id: player_id,
                            position: spawn_at.clone(),
                        }))
                        .unwrap();
                }
            }
        }
    }
}

pub struct GameHandle {
    state: Arc<RwLock<GameState>>,
}

impl GameHandle {
    pub async fn player_count(&self) -> shared::PlayerId {
        let lock = self.state.read().await;
        lock.player_count
    }

    pub async fn add_player(&self, ws_sender: UnboundedSender<WsMessage>) -> shared::PlayerId {
        let mut lock = self.state.write().await;
        lock.player_count += 1;
        let player_id = lock.next_player_id;
        lock.next_player_id += 1;

        lock.send
            .send(GameAction::Join {
                player_id,
                ws_sender,
            })
            .unwrap();

        player_id
    }

    pub async fn remove_player(&self, player_id: shared::PlayerId) {
        let lock = self.state.read().await;
        let _ = lock.send.send(GameAction::Leave { player_id });
    }
}
