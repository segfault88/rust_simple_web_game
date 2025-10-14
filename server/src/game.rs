use anyhow::Result;
use shared::{ClientToServerWsMessage, OtherPlayer, PlayerId, PlayerState, Position};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::{RwLock, RwLockWriteGuard};
use tokio::time::{Duration, interval};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

const TICKS_PER_SECOND: u64 = 30;

#[derive(Clone)]
pub enum WsMessage {
    Kick,
    Spawn(Position, Vec<OtherPlayer>),
    PlayerSpawn(OtherPlayer),
    Leave(PlayerId),
    /// player starts moving, include server's current position as a quick hack to re-sync a bit
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
            position: self.position,
            moving_to,
        }
    }

    pub fn new_joined_player(player_id: PlayerId, ws_sender: UnboundedSender<WsMessage>) -> Player {
        Player {
            player_id,
            state: PlayerState::BeforeSpawn,
            position: Position::new(player_id),
            moving_to: None,
            ws_handler: ws_sender,
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

        match handle_inbound(&mut lock) {
            Ok(_) => {}
            Err(err) => {
                warn!("error handling inbound: {}", err);
            }
        }

        // look for players that need to be spawned and spawn them
        let players_to_spawn: Vec<(u16, Position)> = lock
            .players
            .iter()
            .filter(|(_, p)| matches!(p.state, PlayerState::BeforeSpawn))
            .map(|(&id, player)| (id, player.position))
            .collect();

        for (player_id, spawn_at) in players_to_spawn {
            // when spawning the player, send info about all the other active
            // players, so prepare the list without the player currently being
            // spawned
            let other_players = lock
                .players
                .iter()
                .filter(|(id, _)| **id != player_id)
                .map(|(_, player)| player.to_other_player())
                .collect();

            match lock.players.get_mut(&player_id) {
                Some(player) => {
                    info!(
                        "spawning player id: {:?} at: {:?}",
                        player_id, player.position
                    );
                    player.state = PlayerState::Alive;

                    // send spawn to this player
                    player
                        .ws_handler
                        .send(WsMessage::Spawn(spawn_at, other_players))
                        .unwrap();
                }
                None => {
                    warn!(
                        "trying to move player from BeforeSpawn to Alive, but player not found player_id: {:?}",
                        player_id
                    )
                }
            }

            // notify other players that this player is spawning
            let _ = broadcast(
                &mut lock,
                Some(player_id),
                WsMessage::PlayerSpawn(OtherPlayer {
                    player_id,
                    position: spawn_at,
                    moving_to: None,
                }),
            )
            .inspect_err(|e| {
                warn!(
                    "broadcast for playerspawn failed player_id: {:?}, err: {:?}",
                    player_id, e
                )
            });
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

fn handle_inbound(lock: &mut RwLockWriteGuard<GameState>) -> Result<()> {
    while let Ok(action) = lock.receive.try_recv() {
        match action {
            GameAction::Join {
                player_id,
                ws_sender,
            } => {
                info!(player_id, "completing join");
                lock.players
                    .insert(player_id, Player::new_joined_player(player_id, ws_sender));
            }
            GameAction::Leave { player_id } => {
                info!(player_id, "completing leave");
                lock.players.remove(&player_id);
                // notify all other players that the player left
                broadcast(lock, Some(player_id), WsMessage::Leave(player_id))?;
            }
            GameAction::StartMoving { player_id, to } => {
                debug!(player_id, "start moving: {:?}", to);

                let moving_player_position = match lock.players.get_mut(&player_id) {
                    Some(player) => {
                        player.moving_to = Some(to);
                        player.position
                    }
                    None => {
                        warn!(
                            "got start moving for player that doesn't exist player id: {:?} to: {:?}",
                            player_id, to
                        );
                        continue;
                    }
                };

                // broadcast player moving to all other players
                broadcast(
                    lock,
                    Some(player_id),
                    WsMessage::PlayerMoving(player_id, moving_player_position, to),
                )?;
            }
        }
    }

    Ok(())
}

/// send a WsMessage to all players, optionally excluding one. For example, when
/// leaving, all players except the one that left should be notified
fn broadcast(
    lock: &mut RwLockWriteGuard<GameState>,
    exclude_player_id: Option<PlayerId>,
    message: WsMessage,
) -> Result<()> {
    for player in lock.players.values_mut() {
        if let Some(exclude) = exclude_player_id
            && player.player_id == exclude
        {
            continue;
        }

        match player.ws_handler.send(message.clone()) {
            Ok(_) => {}
            Err(error) => {
                warn!(
                    "failed to broadcast player message to player_id: {:?}, message: {:?}",
                    player.player_id, error
                );
            }
        }
    }

    Ok(())
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
