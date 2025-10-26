use crate::{
    console_log,
    websocket::{ConnectionState, WebSocketHandler},
    world::{self, screen_space_to_world_space, world_space_to_screen_space},
};
use anyhow::{Context, Result};
use shared::{ClientToServerWsMessage, OtherPlayer, PlayerId, Position, ServerToClientWsMessage};
use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
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

            console_log!("canvas_click: {:.2?}, to: {:.2?}", position, to);

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
                        "spawning at {:.2?} with {} other players",
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
                        "player {} spawned at {:.2?}",
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
                                "other player started moving id: {:?}, from: {:.2?}, to: {:.2?}",
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
                                "got player started moving for player that doesn't exist, ignoring id: {:?}, to: {:.2?}",
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
        let (width, height) = (size.width as f64, size.height as f64);

        let ws_state = self.ws_handler.borrow().state();

        let ctx = &self.ctx;

        ctx.set_fill_style_str(render_settings::BACKGROUND);
        ctx.fill_rect(0.0, 0.0, size.width as f64, size.height as f64);

        if ConnectionState::Connected == ws_state
            && let Some(position) = &self.position
        {
            // draw the background grid
            ctx.set_fill_style_str(render_settings::FILL);

            let draw_x_line = |line_at_x: f64| -> u64 {
                ctx.begin_path();
                ctx.move_to(line_at_x, 0.0);
                ctx.line_to(line_at_x, height);
                ctx.stroke();
                0
            };

            let start_world_x = (position.x / render_settings::GRID_SIZE).floor();
            let mut current_world_x = start_world_x;

            loop {
                let (current_x, _) = world_space_to_screen_space(
                    position,
                    &Position {
                        x: current_world_x,
                        y: 0.0,
                    },
                    width,
                    height,
                );

                if current_x <= 0.0 {
                    break;
                }

                draw_x_line(current_x);

                current_world_x -= render_settings::GRID_SIZE;
            }

            current_world_x = start_world_x + render_settings::GRID_SIZE;

            loop {
                let (current_x, _) = world_space_to_screen_space(
                    position,
                    &Position {
                        x: current_world_x,
                        y: 0.0,
                    },
                    width,
                    height,
                );

                if current_x >= width {
                    break;
                }

                draw_x_line(current_x);

                current_world_x += render_settings::GRID_SIZE;
            }

            let draw_y_line = |line_at_y: f64| -> u64 {
                ctx.begin_path();
                ctx.move_to(0.0, line_at_y);
                ctx.line_to(width, line_at_y);
                ctx.stroke();
                0
            };

            let start_world_y = (position.y / render_settings::GRID_SIZE).floor();
            let mut current_world_y = start_world_y;

            loop {
                let (_, current_y) = world_space_to_screen_space(
                    position,
                    &Position {
                        x: 0.0,
                        y: current_world_y,
                    },
                    width,
                    height,
                );

                if current_y <= 0.0 {
                    break;
                }

                draw_y_line(current_y);

                current_world_y -= render_settings::GRID_SIZE;
            }

            current_world_y = start_world_y + render_settings::GRID_SIZE;

            loop {
                let (_, current_y) = world_space_to_screen_space(
                    position,
                    &Position {
                        x: 0.0,
                        y: current_world_y,
                    },
                    width,
                    height,
                );

                if current_y >= height {
                    break;
                }

                draw_y_line(current_y);

                current_world_y += render_settings::GRID_SIZE;
            }

            // draw the player circle
            ctx.set_fill_style_str(render_settings::PLAYER);

            ctx.begin_path();
            ctx.arc(
                width / 2.0,
                height / 2.0,
                render_settings::PLAYER_RADIUS,
                0.0,
                2.0 * PI,
            )
            .unwrap();
            ctx.fill();
            ctx.close_path();

            // draw other players
            ctx.set_fill_style_str(render_settings::OTHER_PLAYER);
            for other_player in self.other_players.values() {
                let (other_x, other_y) =
                    world_space_to_screen_space(position, &other_player.position, width, height);

                ctx.begin_path();
                ctx.arc(
                    other_x,
                    other_y,
                    render_settings::PLAYER_RADIUS,
                    0.0,
                    2.0 * PI,
                )
                .unwrap();
                ctx.fill();
                ctx.close_path();
            }

            ctx.set_fill_style_str(render_settings::FILL);
        } else {
            ctx.set_fill_style_str(render_settings::ERROR);
            ctx.fill_rect(0.0, 0.0, size.width.into(), size.height.into());
            ctx.set_fill_style_str(render_settings::FILL);
        }

        // draw text showing connection state and player info
        ctx.set_font("30px \"Inter\", sans-serif");

        let text = format!(
            "State: {:?} | Player Id: {:?} | Position: {:.2?}",
            ws_state,
            self.player_id,
            self.position.unwrap_or_default()
        );
        ctx.fill_text(text.as_str(), 50.0, 50.0).unwrap();
        let text = format!("Other players: {}", self.other_players.len());
        ctx.fill_text(&text, 50.0, 100.0).unwrap();
    }
}

mod render_settings {
    pub const BACKGROUND: &str = "#fff";
    pub const FILL: &str = "#333";
    pub const PLAYER: &str = "#333388";
    pub const OTHER_PLAYER: &str = "#883333";
    pub const ERROR: &str = "#ff6961";
    pub const GRID_SIZE: f64 = 25.0;
    pub const PLAYER_RADIUS: f64 = 25.0;
}
