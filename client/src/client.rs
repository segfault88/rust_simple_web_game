use crate::{
    console_log,
    websocket::WebSocketHandler,
    world::{self, screen_space_to_world_space},
};
use anyhow::{Context, Result};
use shared::{ClientToServerWsMessage, OtherPlayer, PlayerId, Position, ServerToClientWsMessage};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use winit::dpi::PhysicalPosition;
use winit::window::Window;

pub struct Client {
    pub window: Window,
    pub canvas: HtmlCanvasElement,
    pub ctx: CanvasRenderingContext2d,
    pub ws_handler: Rc<RefCell<WebSocketHandler>>,

    // Game state
    pub player_id: Option<PlayerId>,
    pub position: Position,
    pub moving_to: Option<Position>,
    pub other_players: Vec<OtherPlayer>,
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
            player_id: None,
            position: Position::default(),
            moving_to: None,
            other_players: Vec::new(),
        }
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn canvas_click(&mut self, position: PhysicalPosition<f64>) {
        let to = screen_space_to_world_space(
            &self.position,
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
                        "Spawned at {:?} with {} other players",
                        position,
                        other_players.len()
                    );
                    self.position = position;
                    self.other_players = other_players;
                }
                ServerToClientWsMessage::PlayerSpawn(other_player) => {
                    console_log!("Player {} spawned", other_player.player_id);
                    self.other_players.push(other_player);
                }
                ServerToClientWsMessage::Leave(player_id) => {
                    console_log!("Player {} left", player_id);
                    self.other_players.retain(|p| p.player_id != player_id);
                }
                ServerToClientWsMessage::PlayerMoving(player_id, position, moving_to) => {
                    if let Some(player) = self
                        .other_players
                        .iter_mut()
                        .find(|p| p.player_id == player_id)
                    {
                        player.position = position;
                        player.moving_to = Some(moving_to);
                    }
                }
            }
        }
    }

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
