use shared::{ClientToServerWsMessage, OtherPlayer, PlayerId, Position};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::time::{Duration, interval};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

const TICKS_PER_SECOND: u64 = 30;

pub enum WsMessage {
    Kick,
    Spawn(Position, Vec<OtherPlayer>),
    PlayerSpawn(OtherPlayer),
    Leave(PlayerId),
    // player starts moving, include server's current position as a quick hack to re-sync a bit
    PlayerMoving(PlayerId, Position, Position),
}

#[derive(Debug)]
struct Player {
    player_id: PlayerId,
    state: shared::PlayerState,
    position: Position,
    moving_to: Option<Position>,
    ws_handler: UnboundedSender<WsMessage>,
}

impl Player {
    pub fn to_other_player(&self) -> OtherPlayer {
        // check if the player has already reached their target
        let moving_to = if let Some(target) = self.moving_to {
            let diff = target - self.position;
            let distance = (diff.x * diff.x + diff.y * diff.y).sqrt();
            if distance < shared::STOP_WHEN_CLOSER_THAN {
                None // Already at target, don't include moving_to
            } else {
                Some(target)
            }
        } else {
            None
        };

        OtherPlayer {
            player_id: self.player_id,
            position: self.position.clone(),
            moving_to,
        }
    }
}

#[derive(Debug)]
pub struct GameState {
    next_player_id: PlayerId,
    send: UnboundedSender<GameAction>,
    receive: UnboundedReceiver<GameAction>,
    players: HashMap<PlayerId, Player>,
    last_frame_time: Instant,
}

impl GameState {
    pub fn new() -> Self {
        let (send, receive) = unbounded_channel::<GameAction>();

        GameState {
            next_player_id: 1,
            send,
            receive,
            players: HashMap::new(),
            last_frame_time: Instant::now(),
        }
    }
}

enum GameAction {
    Join {
        player_id: PlayerId,
        ws_sender: UnboundedSender<WsMessage>,
    },
    Leave {
        player_id: PlayerId,
    },
    StartMoving {
        player_id: PlayerId,
        to: Position,
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

        let tick_interval = Duration::from_micros(1_000_000u64 / TICKS_PER_SECOND);
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
                            player_id,
                            state: shared::PlayerState::BeforeSpawn,
                            position: Position::new(player_id),
                            moving_to: None,
                            ws_handler: ws_sender,
                        },
                    );
                }
                GameAction::Leave { player_id } => {
                    info!(player_id, "completing leave");
                    lock.players.remove(&player_id);
                    // notify all other players that the player left
                    for player in lock.players.values() {
                        if let Err(error) = player.ws_handler.send(WsMessage::Leave(player_id)) {
                            warn!(
                                "failed to send leave message for player id: {:?} to player id: {:?}, maybe they are gone? err: {:?}",
                                player_id, player.player_id, error
                            );
                        }
                    }
                }
                GameAction::StartMoving { player_id, to } => {
                    debug!(player_id, "start moving: {:?}", to);

                    let moving_player_position = match lock.players.get_mut(&player_id) {
                        Some(player) => {
                            player.moving_to = Some(to.clone());
                            player.position.clone()
                        }
                        None => {
                            warn!(
                                "got start moving for player that doesn't exist player id: {:?} to: {:?}",
                                player_id, to
                            );
                            return;
                        }
                    };

                    // broadcast to other players
                    for player in lock.players.values_mut() {
                        if player.player_id == player_id {
                            continue;
                        }

                        match player.ws_handler.send(WsMessage::PlayerMoving(
                            player_id,
                            moving_player_position.clone(),
                            to.clone(),
                        )) {
                            Ok(_) => {}
                            Err(error) => {
                                warn!(
                                    "failed to broadcast player moving for player: {:?} to: {:?}, error: {:?}",
                                    player_id, player.player_id, error,
                                );
                            }
                        }
                    }
                }
            }
        }

        // collect players that need to be spawned
        let players_to_spawn: Vec<(PlayerId, Position)> = lock
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
                .filter(|p| {
                    p.player_id != player_id && matches!(p.state, shared::PlayerState::Alive)
                })
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
            for player in lock.players.values() {
                if player.player_id != player_id
                    && matches!(player.state, shared::PlayerState::Alive)
                {
                    player
                        .ws_handler
                        .send(WsMessage::PlayerSpawn(OtherPlayer {
                            player_id,
                            position: spawn_at.clone(),
                            moving_to: None,
                        }))
                        .unwrap();
                }
            }
        }

        // move players
        let now = Instant::now();
        let update_time = now - lock.last_frame_time;

        for player in lock.players.values_mut() {
            if let Some(target) = player.moving_to {
                let (new_pos, distance) =
                    shared::update_pos_move(player.position, target, update_time);

                player.position = new_pos;
                if distance < shared::STOP_WHEN_CLOSER_THAN {
                    // reached target, stop
                    player.moving_to = None;
                }
            }
        }
        lock.last_frame_time = now;
    }
}

pub struct GameHandle {
    state: Arc<RwLock<GameState>>,
}

impl GameHandle {
    pub async fn player_count(&self) -> u16 {
        let lock = self.state.read().await;
        lock.players.len() as u16
    }

    pub async fn add_player(&self, ws_sender: UnboundedSender<WsMessage>) -> PlayerId {
        let mut lock = self.state.write().await;
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

    pub async fn remove_player(&self, player_id: PlayerId) {
        let lock = self.state.read().await;
        let _ = lock.send.send(GameAction::Leave { player_id });
    }

    pub async fn client_message(&self, player_id: PlayerId, message: ClientToServerWsMessage) {
        let game_message = match message {
            ClientToServerWsMessage::StartMoving(to) => GameAction::StartMoving { player_id, to },
        };

        let lock = self.state.read().await;
        let _ = lock.send.send(game_message);
    }
}
