use tokio_util::sync::CancellationToken;

pub struct Game {
    player_count: usize,
}

impl Game {
    pub fn new() -> Self {
        Self { player_count: 0 }
    }

    pub async fn run(&mut self, cancel: CancellationToken) {
        println!("Game running");

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    println!("Game cancelled");
                    break;
                }
            }
        }

        println!("Game shutdown");
    }

    pub fn player_count(&self) -> usize {
        self.player_count
    }
}
