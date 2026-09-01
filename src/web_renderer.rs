use js_sys::Float32Array;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::renderer::{RenderAgent, RenderBlock, RenderPad, RenderPalette, Renderer};

const BLOCK_STRIDE: usize = 10;
const PAD_STRIDE: usize = 8;
const AGENT_STRIDE: usize = 22;

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

    #[allow(clippy::too_many_arguments)]
    pub fn set_scene(
        &mut self,
        blocks: &Float32Array,
        pads: &Float32Array,
        agents: &Float32Array,
        player: &Float32Array,
        palette: &Float32Array,
        ground_size: f32,
        camera: &Float32Array,
        elapsed: f32,
    ) {
        let blocks = blocks.to_vec();
        let pads = pads.to_vec();
        let agents = agents.to_vec();
        let player = player.to_vec();
        let palette = palette.to_vec();
        let camera = camera.to_vec();

        let blocks = blocks
            .chunks_exact(BLOCK_STRIDE)
            .map(|values| RenderBlock {
                position: [values[0], values[1], values[2]],
                size: [values[3], values[4], values[5]],
                color: [values[6], values[7], values[8], values[9]],
            })
            .collect::<Vec<_>>();
        let pads = pads
            .chunks_exact(PAD_STRIDE)
            .map(|values| RenderPad {
                x: values[0],
                z: values[1],
                radius: values[2],
                seconds: values[3],
                color: [values[4], values[5], values[6], values[7]],
            })
            .collect::<Vec<_>>();
        let agents = agents
            .chunks_exact(AGENT_STRIDE)
            .map(render_agent)
            .collect::<Vec<_>>();

        self.renderer.set_scene(
            &blocks,
            &pads,
            &agents,
            render_agent(&player),
            ground_size,
            RenderPalette {
                sky: [palette[0], palette[1], palette[2], palette[3]],
                ground: [palette[4], palette[5], palette[6], palette[7]],
                ground_edge: [palette[8], palette[9], palette[10], palette[11]],
                grid: [palette[12], palette[13], palette[14], palette[15]],
                ink: [palette[16], palette[17], palette[18], palette[19]],
            },
            [
                camera.first().copied().unwrap_or(0.0),
                camera.get(1).copied().unwrap_or(0.0),
                camera.get(2).copied().unwrap_or(8.0),
            ],
            elapsed,
        );
    }

    pub fn draw(&mut self) {
        self.renderer.draw();
    }
}

fn render_agent(values: &[f32]) -> RenderAgent {
    let value = |index: usize| values.get(index).copied().unwrap_or(0.0);
    RenderAgent {
        position: [value(0), value(1), value(2)],
        yaw: value(3),
        walk_cycle: value(4),
        assembled: value(5),
        skin: [value(6), value(7), value(8), value(9)],
        shirt: [value(10), value(11), value(12), value(13)],
        pants: [value(14), value(15), value(16), value(17)],
        shoes: [value(18), value(19), value(20), value(21)],
    }
}
