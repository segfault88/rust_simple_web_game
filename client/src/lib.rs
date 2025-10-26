use anyhow::{Context, Result};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};
use winit::{
    application::ApplicationHandler,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};

#[cfg(target_arch = "wasm32")]
use winit::platform::web::{EventLoopExtWebSys, WindowExtWebSys};

mod console;

struct State {
    window: Window,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // only create canvas and setup once
        if self.state.is_some() {
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
        if let Some(state) = &self.state {
            match event {
                WindowEvent::RedrawRequested => {
                    let size = state.window.inner_size();
                    console_log!("inner_size size: {:?}", size);

                    state
                        .ctx
                        .clear_rect(0.0, 0.0, size.width as f64, size.height as f64);
                }
                WindowEvent::Resized(size) => {
                    state.canvas.set_width(size.width);
                    state.canvas.set_height(size.height);
                    state.window.request_redraw();
                }
                WindowEvent::CursorEntered { device_id: _ }
                | WindowEvent::CursorLeft { device_id: _ }
                | WindowEvent::CursorMoved {
                    device_id: _,
                    position: _,
                }
                | WindowEvent::MouseInput {
                    device_id: _,
                    state: _,
                    button: _,
                } => {}
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

        let window_attributes = WindowAttributes::default();
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

        // store window, canvas, ctx and request_redraw now
        self.state = Some(State {
            window,
            canvas,
            ctx,
        });
        self.state.as_ref().unwrap().window.request_redraw();
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
