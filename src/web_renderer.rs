use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::engine::Engine;
use crate::renderer::Renderer;

#[wasm_bindgen]
pub struct WebRenderer {
    renderer: Renderer,
}

#[wasm_bindgen]
impl WebRenderer {
    pub async fn create(
        canvas: HtmlCanvasElement,
        width: f32,
        height: f32,
    ) -> Result<WebRenderer, JsValue> {
        Ok(WebRenderer {
            renderer: Renderer::new_web(canvas, width, height).await?,
        })
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.renderer.resize(width, height);
    }

    /// Selects the staged character visual rollout mode: 0 = legacy, 1 =
    /// magic. Invalid values are rejected without changing the current mode.
    pub fn set_appearance_mode(&mut self, mode: u8) -> bool {
        let Some(mode) = crate::renderer::CharacterRenderMode::from_u8(mode) else {
            return false;
        };
        self.renderer.set_character_render_mode(mode);
        true
    }

    pub fn appearance_mode(&self) -> u8 {
        self.renderer.character_render_mode().as_u8()
    }

    pub fn sync_engine(&mut self, engine: usize) {
        let engine = engine as *const Engine;
        if let Some(engine) = unsafe { engine.as_ref() } {
            self.renderer.sync_engine(engine);
        }
    }

    pub fn draw(&mut self) {
        self.renderer.draw();
    }
}
