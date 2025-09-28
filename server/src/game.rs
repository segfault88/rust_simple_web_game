use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

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
        println!("Game loop starting");

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    println!("Game loop canceled");
                    break
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(1000)) => {
                    self.game_tick().await;
                }
            }
        }
    }

    pub async fn game_tick(&self) {
        let lock = self.state.write().await;

        println!("Game tick players: {}", lock.player_count);
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
