use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

pub type PlayerId = u16;

#[derive(Serialize, Deserialize, Encode, Decode, Debug)]
pub enum ClientWsMessage {
    Joined(PlayerId),
}
