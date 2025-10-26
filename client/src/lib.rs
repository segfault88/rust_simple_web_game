mod client;
mod console;
mod world;

use anyhow::{Context, Result};
use client::Client;
use shared::Position;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, WebSocket};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};

#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowExtWebSys};

#[derive(Default)]
struct App {
    client: Option<Client>,
    cursor: PhysicalPosition<f64>,
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // only create canvas and setup once
        if self.client.is_some() {
            console_log!("app resumed: already setup, continuing");
            return;
        }

        console_log!("app resumed: setting up window, canvas and ctx");

        self.setup_canvas(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let Some(client) = &mut self.client {
            match event {
                WindowEvent::RedrawRequested => {
                    let size = client.window.inner_size();

                    let ctx = &client.ctx;

                    // set background to white
                    ctx.set_fill_style(&"white".into());
                    ctx.fill_rect(0.0, 0.0, size.width as f64, size.height as f64);

                    // draw text
                    ctx.set_fill_style(&"black".into());
                    ctx.set_font("bold 32px Inter, sans-serif");
                    let text = format!("WTF seems like we can <stuff>");
                    ctx.fill_text(text.as_str(), 50 as f64, 50 as f64).unwrap();

                    // we need to request a redraw to get a continuous loop
                    client.window.request_redraw();
                }
                WindowEvent::Resized(size) => {
                    client.canvas.set_width(size.width);
                    client.canvas.set_height(size.height);
                    client.window.request_redraw();
                }
                WindowEvent::CursorEntered { device_id: _ }
                | WindowEvent::CursorLeft { device_id: _ } => {}
                WindowEvent::CursorMoved {
                    device_id: _,
                    position,
                } => {
                    self.cursor = position;
                }
                WindowEvent::MouseInput {
                    device_id: _,
                    state,
                    button,
                } => match (state, button) {
                    (ElementState::Pressed, MouseButton::Left) => {
                        client.canvas_click(self.cursor);
                    }
                    _ => {}
                },
                _ => {
                    console_log!("unhandled window_event: {:?}", event);
                }
            }
        } else {
            console_log!("window_event but no state, skipping");
        }
    }
}

impl App {
    fn setup_canvas(&mut self, event_loop: &ActiveEventLoop) {
        let js_window = web_sys::window().expect("no global `window` exists");
        let document = js_window
            .document()
            .expect("should have a document on window");
        let container = document
            .get_element_by_id("winit-container")
            .expect("winit-container element not found in the DOM");

        let inner_size = {
            let width = js_window.inner_width().unwrap().as_f64().unwrap() as u32;
            let height = js_window.inner_height().unwrap().as_f64().unwrap() as u32;
            winit::dpi::PhysicalSize::new(width, height)
        };

        let window_attributes = WindowAttributes::default().with_inner_size(inner_size);
        let window = event_loop
            .create_window(window_attributes)
            .expect("unable to create winit window");

        #[cfg(target_arch = "wasm32")]
        let canvas = window
            .canvas()
            .expect("expected WindowExtWebSys to create canvas");
        #[cfg(not(target_arch = "wasm32"))]
        let canvas = document
            .create_element("canvas")
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();

        container.append_child(&canvas).unwrap();

        canvas.focus().unwrap();

        let ctx = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();

        // build client, save state
        self.client = Some(Client::new(window, canvas, ctx));
        // request draw now
        if let Some(c) = &self.client {
            c.request_redraw();
        }
    }
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();

    let event_loop = EventLoop::builder().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    #[cfg(target_arch = "wasm32")]
    {
        // allocate on the heap and leak to get a 'static reference
        let app = Box::new(App::default());
        let app_ref: &'static mut App = Box::leak(app);

        // in wasm, this kicks off the async loop, so needs the app to always
        // exist - at least that's what I think it's trying to say
        event_loop.spawn_app(app_ref);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // not really implemented for native, but just leaving the stub and
        // keeping rust-analyzer happy
        let mut app = App::default();

        event_loop.run_app(&mut app).unwrap();
    }
}
