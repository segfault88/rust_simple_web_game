use bincode::{Decode, Encode};
use std::ops::Sub;
use std::time::Duration;

pub type PlayerId = u16;

#[derive(Encode, Decode, Debug)]
pub enum ServerToClientWsMessage {
    Joined(PlayerId),
    // Spawn this player
    Spawn(Position, Vec<OtherPlayer>),
    // Spawning other players
    PlayerSpawn(OtherPlayer),
    // Other play left / disconnected
    Leave(PlayerId),
    // Other player started moving (for example, by clicking on their canvas),
    // includes current server position as a quick hack to stop desync in this
    // silly little prototype
    PlayerMoving(PlayerId, Position, Position),
}

#[derive(Encode, Decode, Debug)]
pub enum ClientToServerWsMessage {
    // Player started moving (by clicking for now)
    StartMoving(Position),
}

#[derive(Encode, Decode, Debug)]
pub enum PlayerState {
    BeforeSpawn,
    Alive,
    Dead,
}

#[derive(Encode, Decode, Debug, Clone, Copy, Default)]
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

    pub fn normalize(&self) -> Position {
        let magnitude = (self.x * self.x + self.y * self.y).sqrt();
        if magnitude <= STOP_WHEN_CLOSER_THAN {
            Position { x: 0.0, y: 0.0 }
        } else {
            Position {
                x: self.x / magnitude,
                y: self.y / magnitude,
            }
        }
    }
}

impl Sub for Position {
    type Output = Self;

    fn sub(self, rhs: Position) -> Position {
        Position {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Sub<&Position> for &Position {
    type Output = Position;

    fn sub(self, rhs: &Position) -> Position {
        Position {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

// world units per second, constant for now
pub const SPEED: f64 = 20.0;
// stop trying to move when distance <= this
pub const STOP_WHEN_CLOSER_THAN: f64 = 0.01;

// calculate move given target position, speed and duration since last frame, returns new position and distance (used to stop)
pub fn update_pos_move(
    from: &Position,
    target: &Position,
    update_time: Duration,
) -> (Position, f64) {
    let diff = target - from;
    let distance_to_target = (diff.x * diff.x + diff.y * diff.y).sqrt();

    // stop here if close enough
    if distance_to_target <= STOP_WHEN_CLOSER_THAN {
        return (target.clone(), 0.0);
    }

    let vec = diff.normalize();
    let move_distance = SPEED * update_time.as_secs_f64();

    // beware of overshoot
    if move_distance >= distance_to_target {
        return (target.clone(), 0.0);
    }

    (
        Position {
            x: from.x + vec.x * move_distance,
            y: from.y + vec.y * move_distance,
        },
        distance_to_target,
    )
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct OtherPlayer {
    pub player_id: PlayerId,
    pub position: Position,
    pub moving_to: Option<Position>,
}
