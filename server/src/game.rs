use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};
use tokio_util::sync::CancellationToken;
use tracing::info;

const TICKS_PER_SECOND: u64 = 1;

#[derive(Debug)]
pub struct GameState {
    player_count: usize,
    next_player_id: usize,
}

impl GameState {
    pub fn new() -> Self {
        GameState {
            player_count: 0,
            next_player_id: 1,
        }
    }
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
        let lock = self.state.write().await;

        info!(lock.player_count, "tick");
    }
}

pub struct GameHandle {
    state: Arc<RwLock<GameState>>,
}

impl GameHandle {
    pub async fn player_count(&self) -> usize {
        let lock = self.state.read().await;
        lock.player_count
    }

    pub async fn add_player(&self) -> usize {
        let mut lock = self.state.write().await;
        lock.player_count += 1;
        let player_id = lock.next_player_id;
        lock.next_player_id += 1;
        player_id
    }
}
