use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    CanvasRenderingContext2d, CloseEvent, ErrorEvent, HtmlCanvasElement, MessageEvent, WebSocket,
    Window,
};

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

struct WebSocketCallbacks {
    _onopen: Closure<dyn FnMut(web_sys::Event)>,
    _onmessage: Closure<dyn FnMut(MessageEvent)>,
    _onclose: Closure<dyn FnMut(CloseEvent)>,
    _onerror: Closure<dyn FnMut(ErrorEvent)>,
}

// The main game client that manages state and rendering
struct GameClient {
    window: Window,
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    frame_count: u64,
    state: GameState,
    ws: Option<WebSocket>,
    ws_callbacks: Option<WebSocketCallbacks>,
}

impl GameClient {
    // Create a new GameClient instance
    fn new() -> Result<Self, JsValue> {
        let window = web_sys::window().expect("no global `window` exists");

        let document = window.document().expect("should have a document on window");

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
            window,
            canvas,
            context,
            frame_count: 0,
            state: GameState::Disconnected,
            ws: None,
            ws_callbacks: None,
        })
    }

    // WebSocket event handlers as methods
    fn on_websocket_open(&mut self, _event: web_sys::Event) {
        console_log!("websocket connection opened");
        self.state = GameState::Running;
    }

    fn on_websocket_message(&mut self, event: MessageEvent) {
        console_log!("websocket message received: {:?}", event.data());
        // TODO: Parse and handle game messages here
        // For example:
        // if let Ok(text) = event.data().dyn_into::<js_sys::JsString>() {
        //     let message = text.as_string().unwrap();
        //     // Handle the message...
        // }
    }

    fn on_websocket_close(&mut self, event: CloseEvent) {
        console_log!(
            "websocket connection closed: code={}, reason={}",
            event.code(),
            event.reason()
        );
        self.state = GameState::Disconnected;
        self.ws = None;
        self.ws_callbacks = None;
    }

    fn on_websocket_error(&mut self, event: ErrorEvent) {
        console_log!("websocket error occurred: {:?}", event);
        self.state = GameState::Disconnected;
    }

    // Connect to the WebSocket server
    // Takes an Rc<RefCell<GameClient>> so callbacks can mutate the client
    fn connect_websocket(client_ref: Rc<RefCell<GameClient>>) -> Result<(), JsValue> {
        let ws_url = {
            let client = client_ref.borrow();
            if client.ws.is_some() {
                return Ok(()); // Already connected or connecting
            }

            // build ws url
            let location = client.window.location();
            let protocol = if location.protocol()? == "https:" {
                "wss:"
            } else {
                "ws:"
            };
            let host = location.host()?;
            format!("{}//{}/ws", protocol, host)
        };

        console_log!("connecting ws: {}", ws_url);

        let ws = WebSocket::new(&ws_url)?;

        // Create event handler closures
        let onopen_callback = {
            let client_clone = client_ref.clone();
            Closure::wrap(Box::new(move |event: web_sys::Event| {
                client_clone.borrow_mut().on_websocket_open(event);
            }) as Box<dyn FnMut(web_sys::Event)>)
        };

        let onmessage_callback = {
            let client_clone = client_ref.clone();
            Closure::wrap(Box::new(move |event: MessageEvent| {
                client_clone.borrow_mut().on_websocket_message(event);
            }) as Box<dyn FnMut(MessageEvent)>)
        };

        let onclose_callback = {
            let client_clone = client_ref.clone();
            Closure::wrap(Box::new(move |event: CloseEvent| {
                client_clone.borrow_mut().on_websocket_close(event);
            }) as Box<dyn FnMut(CloseEvent)>)
        };

        let onerror_callback = {
            let client_clone = client_ref.clone();
            Closure::wrap(Box::new(move |event: ErrorEvent| {
                client_clone.borrow_mut().on_websocket_error(event);
            }) as Box<dyn FnMut(ErrorEvent)>)
        };

        // Set the callbacks on the WebSocket
        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        ws.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));

        // Store the websocket, callbacks, and update state
        let mut client = client_ref.borrow_mut();
        client.state = GameState::Connecting;
        client.ws = Some(ws);
        client.ws_callbacks = Some(WebSocketCallbacks {
            _onopen: onopen_callback,
            _onmessage: onmessage_callback,
            _onclose: onclose_callback,
            _onerror: onerror_callback,
        });

        Ok(())
    }

    // Send a message through the WebSocket
    fn _send_message(&self, message: &str) -> Result<(), JsValue> {
        if let Some(ref ws) = self.ws {
            if ws.ready_state() == WebSocket::OPEN {
                ws.send_with_str(message)?;
                console_log!("sent WebSocket message: {}", message);
            } else {
                console_log!("webSocket not open, cannot send message");
            }
        }
        Ok(())
    }

    // Update game state (called each frame)
    fn update(&mut self) {
        self.frame_count += 1;

        // Game logic will go here
    }

    // Render the current game state
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
    }
}

// Request the next animation frame with the given closure
fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .expect("no global `window` exists")
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}

#[wasm_bindgen(start)]
fn run() {
    console_log!("starting game client...");

    let client = Rc::new(RefCell::new(
        GameClient::new().expect("Failed to create GameClient"),
    ));

    // Connect to WebSocket
    GameClient::connect_websocket(client.clone()).expect("Failed to connect to WebSocket");

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
