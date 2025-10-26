use crate::{
    console_log,
    world::{self, screen_space_to_world_space},
};
use anyhow::{Context, Result};
use shared::Position;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, WebSocket};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};

#[derive(Debug)]
pub enum State {
    Disconnected,
    Connecting,
    Running,
}

pub struct Client {
    pub window: Window,
    pub canvas: HtmlCanvasElement,
    pub ctx: CanvasRenderingContext2d,
    pub state: State,
    position: Position,
}

impl Client {
    pub fn new(window: Window, canvas: HtmlCanvasElement, ctx: CanvasRenderingContext2d) -> Client {
        let c = Client {
            window: window,
            canvas: canvas,
            ctx: ctx,
            state: State::Disconnected,
            position: Position::default(),
        };

        c
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    pub fn canvas_click(&self, position: PhysicalPosition<f64>) {
        let to = screen_space_to_world_space(
            &self.position,
            position.x,
            position.y,
            self.canvas.width().into(),
            self.canvas.height().into(),
        );

        console_log!("canvas_click: {:?}, to: {:?}", position, to);
    }

    pub fn connect(&mut self) {}
}
