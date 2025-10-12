use shared::{ClientToServerWsMessage, OtherPlayer, Position, ServerToClientWsMessage};
use std::cell::RefCell;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::Duration;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    CanvasRenderingContext2d, CloseEvent, ErrorEvent, HtmlCanvasElement, MessageEvent, MouseEvent,
    WebSocket, Window,
};

mod console;

const FILL_COLOR: &'static str = "#333";
const PLAYER_COLOR: &'static str = "#333388";
const OTHER_PLAYER_COLOR: &'static str = "#883333";
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

struct CanvasCallbacks {
    _onclick: Closure<dyn FnMut(MouseEvent)>,
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
    canvas_callbacks: Option<CanvasCallbacks>,
    player_id: Option<shared::PlayerId>,
    position: Option<shared::Position>,
    moving_to: Option<shared::Position>,
    other_players: HashMap<shared::PlayerId, OtherPlayer>,
    last_frame_time: f64, // milliseconds from performance.now()
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

        let initial_time = window.performance().unwrap().now();

        Ok(GameClient {
            window,
            canvas,
            context,
            frame_count: 0,
            state: GameState::Disconnected,
            ws: None,
            ws_callbacks: None,
            canvas_callbacks: None,
            player_id: None,
            position: None,
            moving_to: None,
            other_players: HashMap::new(),
            last_frame_time: initial_time,
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
        let message: ServerToClientWsMessage =
            match bincode::decode_from_slice(&bytes, bincode::config::standard()) {
                Ok((msg, _size)) => msg,
                Err(e) => {
                    console_log!("failed to decode message: {:?}", e);
                    return;
                }
            };

        // Handle the message
        match message {
            ServerToClientWsMessage::Joined(player_id) => {
                console_log!("joined game as player {}", player_id);
                self.player_id = Some(player_id);
            }
            ServerToClientWsMessage::Spawn(position, other_players) => {
                console_log!("spawning at {:?}", position);
                self.position = Some(position);

                // save the other players
                self.other_players.clear();
                for other_player in &other_players {
                    self.other_players
                        .insert(other_player.player_id, other_player.clone());
                }
            }
            ServerToClientWsMessage::PlayerSpawn(other_player) => {
                console_log!(
                    "spawning other player id: {:?} at {:?}",
                    other_player.player_id,
                    other_player.position
                );
                self.other_players
                    .insert(other_player.player_id, other_player.clone());
            }
            ServerToClientWsMessage::Leave(player_id) => {
                console_log!("player left, removing id: {:?}", player_id);
                if self.other_players.remove(&player_id).is_none() {
                    console_log!("attempted to remove player not in other_players map");
                }
            }
            ServerToClientWsMessage::PlayerMoving(player_id, from, to) => {
                match self.other_players.get_mut(&player_id) {
                    Some(player) => {
                        console_log!(
                            "other player started moving id: {:?}, from: {:?}, to: {:?}",
                            player_id,
                            from,
                            to
                        );

                        // overwrite current position to match what the server says
                        player.position = from;
                        player.moving_to = Some(to.clone());
                    }
                    None => {
                        console_log!(
                            "got player started moving for player that doesn't exist, ignoring id: {:?}, to: {:?}",
                            player_id,
                            to
                        );
                    }
                }
            }
        }
    }

    fn on_websocket_close(&mut self, event: CloseEvent) {
        console_log!(
            "websocket connection closed: code={}, reason={}",
            event.code(),
            event.reason()
        );
        // this sucks, restructure so not needed
        self.state = GameState::Disconnected;
        self.player_id = None;
        self.position = None;
        self.ws = None;
        self.ws_callbacks = None;
        self.other_players.clear();
    }

    fn on_websocket_error(&mut self, event: ErrorEvent) {
        console_log!("websocket error occurred: {:?}", event);
        // this sucks, restructure so not needed
        self.state = GameState::Disconnected;
        self.player_id = None;
        self.position = None;
        self.ws = None;
        self.ws_callbacks = None;
        self.other_players.clear();
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

    // Handle canvas click event
    fn on_canvas_click(&mut self, event: MouseEvent) {
        match &self.position {
            Some(position) => {
                let width = self.canvas.width() as f64;
                let height = self.canvas.height() as f64;

                let x = event.offset_x() as f64;
                let y = event.offset_y() as f64;

                // TODO: implement click handling logic
                let world_position = screen_space_to_world_space(position, x, y, width, height);

                console_log!(
                    "canvas clicked at: ({}, {}), world: {:?}",
                    x,
                    y,
                    world_position
                );

                _ = self.send_message(&ClientToServerWsMessage::StartMoving(
                    world_position.clone(),
                ));

                // start moving on the client immediately
                self.moving_to = Some(world_position);
            }
            _ => {
                console_log!("click but no possition, doing nothing");
            }
        }
    }

    // Set up canvas event listeners
    fn setup_canvas_listeners(client_ref: Rc<RefCell<GameClient>>) -> Result<(), JsValue> {
        let canvas = {
            let client = client_ref.borrow();
            client.canvas.clone()
        };

        // Create click event handler
        let onclick_callback = {
            let client_clone = client_ref.clone();
            Closure::wrap(Box::new(move |event: MouseEvent| {
                client_clone.borrow_mut().on_canvas_click(event);
            }) as Box<dyn FnMut(MouseEvent)>)
        };

        // Attach the event listener to the canvas
        canvas
            .add_event_listener_with_callback("click", onclick_callback.as_ref().unchecked_ref())?;

        // Store the callback so it doesn't get dropped
        let mut client = client_ref.borrow_mut();
        client.canvas_callbacks = Some(CanvasCallbacks {
            _onclick: onclick_callback,
        });

        Ok(())
    }

    // Send a message through the WebSocket
    fn send_message(&self, message: &ClientToServerWsMessage) -> Result<(), JsValue> {
        if let Some(ref ws) = self.ws {
            if ws.ready_state() == WebSocket::OPEN {
                // encode the bincode message

                let bytes = match bincode::encode_to_vec(message, bincode::config::standard()) {
                    Ok(msg) => msg,
                    Err(e) => {
                        console_log!("failed to encode message: {:?}", e);
                        return Err("failed to encode".into());
                    }
                };

                ws.send_with_u8_array(&bytes)?;
            } else {
                console_log!("websocket not open, cannot send message {:?}", message);
            }
        }
        Ok(())
    }

    // Update game state (called each frame)
    fn update(&mut self) {
        self.frame_count += 1;

        let now = self.window.performance().unwrap().now();
        let since_ms = now - self.last_frame_time;
        let since = Duration::from_secs_f64(since_ms / 1000.0);

        if let (Some(to), Some(from)) = (&self.moving_to, &self.position) {
            // moving current player
            let (new_pos, distance) = shared::update_pos_move(from, to, since);
            self.position = Some(new_pos);
            if distance == 0.0 {
                self.moving_to = None;
            }
        }

        for other_player in self.other_players.values_mut() {
            if let (Some(to), from) = (&other_player.moving_to, &other_player.position) {
                // move other player
                let (new_pos, distance) = shared::update_pos_move(from, to, since);
                other_player.position = new_pos;
                if distance == 0.0 {
                    other_player.moving_to = None;
                }
            }
        }

        self.last_frame_time = now;
    }

    // Render the current game state
    fn render(&self) {
        let width = self.canvas.width() as f64;
        let height = self.canvas.height() as f64;

        let ctx = &self.context;

        // Clear the canvas
        ctx.clear_rect(0.0, 0.0, width, height);

        ctx.set_fill_style_str(FILL_COLOR);

        let mut draw_disconnected = true;

        match self.state {
            GameState::Running => match &self.position {
                Some(position) => {
                    draw_disconnected = false;

                    // draw player circle
                    ctx.set_fill_style_str(PLAYER_COLOR);

                    ctx.begin_path();
                    ctx.arc(width / 2.0, height / 2.0, 10.0, 0.0, 2.0 * PI)
                        .unwrap();
                    ctx.fill();
                    ctx.close_path();

                    ctx.set_fill_style_str(OTHER_PLAYER_COLOR);

                    // draw other players
                    for other_player in self.other_players.values() {
                        let (other_x, other_y) = world_space_to_screen_space(
                            position,
                            &other_player.position,
                            width,
                            height,
                        );

                        ctx.begin_path();
                        ctx.arc(other_x, other_y, 10.0, 0.0, 2.0 * PI).unwrap();
                        ctx.fill();
                        ctx.close_path();
                    }

                    ctx.set_fill_style_str(FILL_COLOR);
                }
                _ => {}
            },
            _ => {}
        }

        if draw_disconnected {
            ctx.set_fill_style_str(ERROR_COLOR);
            ctx.fill_rect(0.0, 0.0, width, height);
            ctx.set_fill_style_str(FILL_COLOR);
        }

        // draw status text
        ctx.set_font("15px sans-serif");
        let text = format!(
            "player_id: {}, state: {:?}",
            self.player_id.unwrap_or_default(),
            self.state,
        );
        ctx.fill_text(&text, 20.0, 20.0).unwrap();
        let position_str = match &(self.position) {
            None => "at: none".into(),
            Some(position) => format!("at: x: {:.2}, y: {:.2}", position.x, position.y),
        };
        ctx.fill_text(&position_str, 20.0, 40.0).unwrap();
        let other_players_str = format!("other players: {}", self.other_players.len());
        ctx.fill_text(&other_players_str, 20.0, 60.0).unwrap();
    }
}

const WORLD_SCALE_FACTOR: f64 = 10.0;

fn world_space_to_screen_space(
    player_position: &Position,
    other: &Position,
    width: f64,
    height: f64,
) -> (f64, f64) {
    // difference in world space
    let dx = other.x - player_position.x;
    let dy = other.y - player_position.y;

    // scale and add screen center offset (current player is always at center of the screen)
    (
        width / 2.0 + dx * WORLD_SCALE_FACTOR,
        height / 2.0 + dy * WORLD_SCALE_FACTOR,
    )
}

fn screen_space_to_world_space(
    player_position: &Position,
    screen_x: f64,
    screen_y: f64,
    width: f64,
    height: f64,
) -> Position {
    // reverse of world_space_to_screen_space
    let world_x = player_position.x + (screen_x - width / 2.0) / WORLD_SCALE_FACTOR;
    let world_y = player_position.y + (screen_y - height / 2.0) / WORLD_SCALE_FACTOR;

    Position {
        x: world_x,
        y: world_y,
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

    // Set up canvas event listeners
    GameClient::setup_canvas_listeners(client.clone()).expect("Failed to setup canvas listeners");

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
