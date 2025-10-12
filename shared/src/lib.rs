use bincode::{Decode, Encode};

pub type PlayerId = u16;

#[derive(Encode, Decode, Debug)]
pub enum ClientWsMessage {
    Joined(PlayerId),
    // Spawn this player
    Spawn(Position, Vec<OtherPlayer>),
    // Spawning other players
    PlayerSpawn(OtherPlayer),
    // Other play left / disconnected
    Leave(PlayerId),
}

impl ClientWsMessage {
    pub fn to_bytes(&self) -> Vec<u8> {
        // todo: clean up unwrap
        bincode::encode_to_vec(self, bincode::config::standard()).unwrap()
    }
}

#[derive(Encode, Decode, Debug)]
pub enum PlayerState {
    BeforeSpawn,
    Alive,
    Dead,
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct Position {
    pub x: f64, // f64 for now, consider fixed point
    pub y: f64,
}

impl Position {
    pub fn new(player_id: PlayerId) -> Position {
        // this is weird, but for now, spawn a player at x = 5 * player_id so they spawn spread out
        Position {
            x: player_id as f64 * 5.0,
            y: 0.0,
        }
    }
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct OtherPlayer {
    pub player_id: PlayerId,
    pub position: Position,
}
