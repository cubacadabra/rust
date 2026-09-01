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
