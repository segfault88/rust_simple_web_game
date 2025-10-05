use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[derive(Debug)]
enum GameState {
    Disconnected,
    Connecting,
    Running,
}

/// The main game client that manages state and rendering
struct GameClient {
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    frame_count: u64,
    state: GameState,
}

impl GameClient {
    /// Create a new GameClient instance
    fn new() -> Result<Self, JsValue> {
        let document = web_sys::window()
            .expect("no global `window` exists")
            .document()
            .expect("should have a document on window");

        let canvas = document
            .get_element_by_id("canvas")
            .expect("canvas element not found");

        let canvas: HtmlCanvasElement = canvas
            .dyn_into::<HtmlCanvasElement>()
            .map_err(|_| JsValue::from_str("Failed to cast to HtmlCanvasElement"))?;

        let context = canvas
            .get_context("2d")?
            .expect("Failed to get 2d context")
            .dyn_into::<CanvasRenderingContext2d>()?;

        Ok(GameClient {
            canvas,
            context,
            frame_count: 0,
            state: GameState::Disconnected,
        })
    }

    /// Update game state (called each frame)
    fn update(&mut self) {
        self.frame_count += 1;
    }

    /// Render the current game state
    fn render(&self) {
        let width = self.canvas.width() as f64;
        let height = self.canvas.height() as f64;

        // Clear the canvas
        self.context.clear_rect(0.0, 0.0, width, height);

        // Draw text with frame counter
        self.context.set_font("15px sans-serif");
        let text = format!(
            "hello from rust state: {:?} frame: {}",
            self.state, self.frame_count
        );
        self.context.fill_text(&text, 150.0, 150.0).unwrap();

        // Draw the smiley face
        self.context.begin_path();

        // Draw the outer circle.
        self.context
            .arc(75.0, 75.0, 50.0, 0.0, std::f64::consts::PI * 2.0)
            .unwrap();

        // Draw the mouth.
        self.context.move_to(110.0, 75.0);
        self.context
            .arc(75.0, 75.0, 35.0, 0.0, std::f64::consts::PI)
            .unwrap();

        // Draw the left eye.
        self.context.move_to(65.0, 65.0);
        self.context
            .arc(60.0, 65.0, 5.0, 0.0, std::f64::consts::PI * 2.0)
            .unwrap();

        // Draw the right eye.
        self.context.move_to(95.0, 65.0);
        self.context
            .arc(90.0, 65.0, 5.0, 0.0, std::f64::consts::PI * 2.0)
            .unwrap();

        self.context.stroke();
    }
}

/// Request the next animation frame with the given closure
fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .expect("no global `window` exists")
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}

#[wasm_bindgen]
pub fn start_game() -> Result<(), JsValue> {
    // not used yet :|
    Ok(())
}

#[wasm_bindgen(start)]
fn run() {
    console_log!("starting game client...");

    let client = Rc::new(RefCell::new(
        GameClient::new().expect("Failed to create GameClient"),
    ));

    // Create the animation loop closure
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();

    let client_clone = client.clone();
    *g.borrow_mut() = Some(Closure::new(move || {
        // Update game state
        client_clone.borrow_mut().update();

        // Render
        client_clone.borrow().render();

        // Request next frame
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    // Start the animation loop
    request_animation_frame(g.borrow().as_ref().unwrap());

    console_log!("startup done");
}
