use crate::{
    console_log,
    websocket::WebSocketHandler,
    world::{self, screen_space_to_world_space},
};
use anyhow::{Context, Result};
use shared::{ClientToServerWsMessage, OtherPlayer, PlayerId, Position, ServerToClientWsMessage};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use web_time::Instant;
use winit::dpi::PhysicalPosition;
use winit::window::Window;

pub struct Client {
    pub window: Window,
    pub canvas: HtmlCanvasElement,
    pub ctx: CanvasRenderingContext2d,
    pub ws_handler: Rc<RefCell<WebSocketHandler>>,

    pub last_frame_time: Instant,

    // Game state
    pub player_id: Option<PlayerId>,
    pub position: Option<Position>,
    pub moving_to: Option<Position>,
    pub other_players: HashMap<PlayerId, OtherPlayer>,
}

impl Client {
    pub fn new(
        window: Window,
        canvas: HtmlCanvasElement,
        ctx: CanvasRenderingContext2d,
        ws_handler: Rc<RefCell<WebSocketHandler>>,
    ) -> Client {
        Client {
            window,
            canvas,
            ctx,
            ws_handler,
            last_frame_time: Instant::now(),
            player_id: None,
            position: None,
            moving_to: None,
            other_players: HashMap::new(),
        }
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn canvas_click(&mut self, position: PhysicalPosition<f64>) {
        if let Some(current_position) = &self.position {
            let to = screen_space_to_world_space(
                current_position,
                position.x,
                position.y,
                self.canvas.width().into(),
                self.canvas.height().into(),
            );

            console_log!("canvas_click: {:?}, to: {:?}", position, to);

            // Send move command to server
            self.moving_to = Some(to);
            if let Err(e) = self
                .ws_handler
                .borrow_mut()
                .send(ClientToServerWsMessage::StartMoving(to))
            {
                console_log!("Failed to send move command: {:?}", e);
            }
        }
    }

    pub fn process_messages(&mut self) {
        let messages = self.ws_handler.borrow_mut().take_pending_messages();

        for msg in messages {
            match msg {
                ServerToClientWsMessage::Joined(player_id) => {
                    console_log!("Joined as player {}", player_id);
                    self.player_id = Some(player_id);
                }
                ServerToClientWsMessage::Spawn(position, other_players) => {
                    console_log!(
                        "spawning at {:?} with {} other players",
                        position,
                        other_players.len()
                    );
                    self.position = Some(position);
                    self.other_players.clear();
                    for other_player in other_players {
                        self.other_players
                            .insert(other_player.player_id, other_player);
                    }
                }
                ServerToClientWsMessage::PlayerSpawn(other_player) => {
                    console_log!(
                        "player {} spawned at {:?}",
                        other_player.player_id,
                        other_player.position
                    );
                    self.other_players
                        .insert(other_player.player_id, other_player);
                }
                ServerToClientWsMessage::Leave(player_id) => {
                    console_log!("player {} left", player_id);
                    self.other_players.remove(&player_id);
                }
                ServerToClientWsMessage::PlayerMoving(player_id, position, moving_to) => {
                    match self.other_players.get_mut(&player_id) {
                        Some(player) => {
                            console_log!(
                                "other player started moving id: {:?}, from: {:?}, to: {:?}",
                                player_id,
                                position,
                                moving_to
                            );

                            // overwrite current position to match what server reports
                            player.position = position;
                            player.moving_to = Some(moving_to);
                        }
                        None => {
                            console_log!(
                                "got player started moving for player that doesn't exist, ignoring id: {:?}, to: {:?}",
                                player_id,
                                moving_to
                            );
                        }
                    }
                }
            }
        }
    }

    /// update game world and state
    pub fn update(&mut self) {
        let now = Instant::now();
        let since = now.duration_since(self.last_frame_time);

        if let (Some(to), Some(from)) = (self.moving_to, self.position) {
            // moving current player
            let (new_pos, distance) = shared::update_pos_move(from, to, since);
            self.position = Some(new_pos);
            if distance < shared::STOP_WHEN_CLOSER_THAN {
                self.moving_to = None;
            }
        }

        for other_player in self.other_players.values_mut() {
            if let (Some(to), from) = (other_player.moving_to, other_player.position) {
                // move other player
                let (new_pos, distance) = shared::update_pos_move(from, to, since);
                other_player.position = new_pos;
                if distance < shared::STOP_WHEN_CLOSER_THAN {
                    other_player.moving_to = None;
                }
            }
        }

        self.last_frame_time = now;
    }

    /// render to canvas
    pub fn render(&mut self) {
        let size = self.window.inner_size();
        let ctx = &self.ctx;

        // set background to white
        ctx.set_fill_style(&"white".into());
        ctx.fill_rect(0.0, 0.0, size.width as f64, size.height as f64);

        // draw text showing connection state and player info
        ctx.set_fill_style(&"black".into());
        ctx.set_font("bold 32px Inter, sans-serif");

        let ws_state = self.ws_handler.borrow().state();
        let text = format!(
            "State: {:?} | Player: {:?} | Others: {}",
            ws_state,
            self.player_id,
            self.other_players.len()
        );
        ctx.fill_text(text.as_str(), 50.0, 50.0).unwrap();
    }
}
