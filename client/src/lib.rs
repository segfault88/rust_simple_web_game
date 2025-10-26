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

        let ctx = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();

        // store window, ctx and request_redraw now
        self.state = Some(State { window, ctx });
        self.state.as_ref().unwrap().window.request_redraw();
    }

    /// Emitted when the OS sends an event to a winit window.
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        console_log!("window_event");
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
        let mut app = App::default();

        event_loop.run_app(&mut app).unwrap();
    }
}
