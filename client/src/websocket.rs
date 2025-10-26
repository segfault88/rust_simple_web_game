use crate::console_log;
use anyhow::Result;
use shared::{ClientToServerWsMessage, ServerToClientWsMessage};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// Shared state that can be accessed by both the app and WebSocket callbacks
pub struct WebSocketHandler {
    ws: Option<WebSocket>,
    state: ConnectionState,
    // Store received messages to be processed in the main game loop
    pending_messages: Vec<ServerToClientWsMessage>,
}

impl WebSocketHandler {
    pub fn new() -> Self {
        Self {
            ws: None,
            state: ConnectionState::Disconnected,
            pending_messages: Vec::new(),
        }
    }

    pub fn state(&self) -> ConnectionState {
        self.state
    }

    pub fn connect(handler: Rc<RefCell<Self>>, url: &str) -> Result<()> {
        console_log!("Connecting to WebSocket at {}", url);

        let ws = WebSocket::new(url)
            .map_err(|e| anyhow::anyhow!("Failed to create WebSocket: {:?}", e))?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        // Clone the Rc for each closure
        let handler_clone = handler.clone();
        let onopen = Closure::wrap(Box::new(move |_event| {
            console_log!("WebSocket connection opened");
            handler_clone.borrow_mut().state = ConnectionState::Connected;
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        onopen.forget(); // Keep the closure alive

        let handler_clone = handler.clone();
        let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
            if let Ok(array_buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() {
                let array = js_sys::Uint8Array::new(&array_buffer);
                let bytes = array.to_vec();

                match bincode::decode_from_slice::<ServerToClientWsMessage, _>(
                    &bytes,
                    bincode::config::standard(),
                ) {
                    Ok((msg, _)) => {
                        console_log!("Received message: {:?}", msg);
                        handler_clone.borrow_mut().pending_messages.push(msg);
                    }
                    Err(e) => {
                        console_log!("Failed to decode message: {:?}", e);
                    }
                }
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();

        let handler_clone = handler.clone();
        let onerror = Closure::wrap(Box::new(move |_event: ErrorEvent| {
            console_log!("WebSocket error occurred");
            handler_clone.borrow_mut().state = ConnectionState::Error;
        }) as Box<dyn FnMut(ErrorEvent)>);
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();

        let handler_clone = handler.clone();
        let onclose = Closure::wrap(Box::new(move |event: CloseEvent| {
            console_log!(
                "WebSocket closed: code={}, reason={}",
                event.code(),
                event.reason()
            );
            handler_clone.borrow_mut().state = ConnectionState::Disconnected;
        }) as Box<dyn FnMut(CloseEvent)>);
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        onclose.forget();

        // Update state and store the WebSocket
        handler.borrow_mut().ws = Some(ws);
        handler.borrow_mut().state = ConnectionState::Connecting;

        Ok(())
    }

    pub fn send(&mut self, msg: ClientToServerWsMessage) -> Result<()> {
        if let Some(ws) = &self.ws {
            let bytes = bincode::encode_to_vec(msg, bincode::config::standard())
                .map_err(|e| anyhow::anyhow!("Failed to encode message: {:?}", e))?;
            ws.send_with_u8_array(&bytes)
                .map_err(|e| anyhow::anyhow!("Failed to send message: {:?}", e))?;
            Ok(())
        } else {
            anyhow::bail!("WebSocket not connected")
        }
    }

    /// Get all pending messages and clear the queue
    pub fn take_pending_messages(&mut self) -> Vec<ServerToClientWsMessage> {
        std::mem::take(&mut self.pending_messages)
    }

    pub fn disconnect(&mut self) {
        if let Some(ws) = self.ws.take() {
            let _ = ws.close();
        }
        self.state = ConnectionState::Disconnected;
    }
}

impl Drop for WebSocketHandler {
    fn drop(&mut self) {
        self.disconnect();
    }
}
