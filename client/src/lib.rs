use shared::OtherPlayer;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    CanvasRenderingContext2d, CloseEvent, ErrorEvent, HtmlCanvasElement, MessageEvent, WebSocket,
    Window,
};

mod console;

const FILL_COLOR: &'static str = "#333";
const ERROR_COLOR: &'static str = "#ff6961";

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
    player_id: Option<shared::PlayerId>,
    position: Option<shared::Position>,
    other_players: HashMap<shared::PlayerId, OtherPlayer>,
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
            player_id: None,
            position: None,
            other_players: HashMap::new(),
        })
    }

    // WebSocket event handlers as methods
    fn on_websocket_open(&mut self, _event: web_sys::Event) {
        console_log!("websocket connection opened");
        self.state = GameState::Running;
    }

    fn on_websocket_message(&mut self, event: MessageEvent) {
        // try to get the data as an ArrayBuffer
        let array_buffer = match event.data().dyn_into::<js_sys::ArrayBuffer>() {
            Ok(buf) => buf,
            Err(_) => {
                console_log!("websocket message is not an ArrayBuffer");
                return;
            }
        };

        // convert ArrayBuffer to Uint8Array to Vec<u8>
        let uint8_array = js_sys::Uint8Array::new(&array_buffer);
        let bytes = uint8_array.to_vec();

        // decode the bincode message
        let message: shared::ClientWsMessage =
            match bincode::decode_from_slice(&bytes, bincode::config::standard()) {
                Ok((msg, _size)) => msg,
                Err(e) => {
                    console_log!("failed to decode message: {:?}", e);
                    return;
                }
            };

        // Handle the message
        match message {
            shared::ClientWsMessage::Joined(player_id) => {
                console_log!("joined game as player {}", player_id);
                self.player_id = Some(player_id);
            }
            shared::ClientWsMessage::Spawn(position, other_players) => {
                console_log!("spawning at {:?}", position);
                self.position = Some(position);

                // save the other players
                self.other_players.clear();
                for other_player in &other_players {
                    self.other_players
                        .insert(other_player.player_id, other_player.clone());
                }
            }
            shared::ClientWsMessage::PlayerSpawn(other_player) => {
                console_log!(
                    "spawning other player id: {:?} at {:?}",
                    other_player.player_id,
                    other_player.position
                );
                self.other_players
                    .insert(other_player.player_id, other_player.clone());
            }
            shared::ClientWsMessage::Leave(player_id) => {
                console_log!("player left, removing id: {:?}", player_id);
                self.other_players.remove(&player_id).unwrap();
            }
        }
    }

    fn on_websocket_close(&mut self, event: CloseEvent) {
        console_log!(
            "websocket connection closed: code={}, reason={}",
            event.code(),
            event.reason()
        );
        self.state = GameState::Disconnected;
        self.player_id = None;
        self.position = None;
        self.ws = None;
        self.ws_callbacks = None;
    }

    fn on_websocket_error(&mut self, event: ErrorEvent) {
        console_log!("websocket error occurred: {:?}", event);
        self.state = GameState::Disconnected;
        self.position = None;
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
        // Set binary type to arraybuffer so we can easily decode binary messages
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

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
                console_log!("sent websocket message: {}", message);
            } else {
                console_log!("websocket not open, cannot send message");
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

        let ctx = &self.context;

        // Clear the canvas
        ctx.clear_rect(0.0, 0.0, width, height);

        ctx.set_fill_style_str(FILL_COLOR);

        let should_draw_error =
            !matches!(self.state, GameState::Running) || self.position.is_none();

        if should_draw_error {
            ctx.set_fill_style_str(ERROR_COLOR);
            ctx.fill_rect(0.0, 0.0, width, height);
        } else {
            // draw player as grey circle
            ctx.begin_path();
            ctx.arc(width / 2.0, height / 2.0, 10.0, 0.0, 6.0).unwrap();
            ctx.fill();
            ctx.close_path();
        }

        ctx.set_fill_style_str(FILL_COLOR);

        // Draw text with frame counter
        ctx.set_font("15px sans-serif");
        let text = format!(
            "player_id: {}, state: {:?} frame: {}",
            self.player_id.unwrap_or_default(),
            self.state,
            self.frame_count
        );
        ctx.fill_text(&text, 50.0, 50.0).unwrap();
        let position_str = match &(self.position) {
            None => "at: none".into(),
            Some(position) => format!("at: x: {}, y: {}", position.x, position.y),
        };
        ctx.fill_text(&position_str, 50.0, 75.0).unwrap();
        let other_players_str = format!("other players: {}", self.other_players.len());
        ctx.fill_text(&other_players_str, 50.0, 100.0).unwrap();
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
