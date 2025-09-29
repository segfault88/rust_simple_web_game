use js_sys::wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn start_game() -> Result<(), JsValue> {
    Ok(())
}

#[wasm_bindgen(start)]
pub fn main() {
}