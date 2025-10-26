use wasm_bindgen::prelude::*;
use winit::{
    application::ApplicationHandler,
    event::{Event, WindowEvent},
    event_loop::ActiveEventLoop,
    event_loop::{ControlFlow, EventLoop},
    window::WindowId,
};

#[cfg(target_arch = "wasm32")]
use winit::platform::web::EventLoopExtWebSys;

mod console;

struct State {}

#[derive(Default)]
struct App {}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        console_log!("resumed");
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

// impl ApplicationHandler<()> for App {}

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
