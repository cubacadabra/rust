const UI_FONT_BYTES: &[u8] = include_bytes!("../../../assets/fonts/LilitaOne-Regular.ttf");

pub(super) const UI_ATLAS_PADDING: u32 = 2;
pub(super) const UI_ATLAS_WIDTH: u32 = 4096;
pub(super) const UI_ATLAS_HEIGHT: u32 = 1312;
pub(super) const UI_FONT_ATLAS_Y: u32 = 1212;
const UI_FONT_ATLAS_SIZE: f32 = 64.0;

fn ui_font() -> &'static Font {
    static FONT: OnceLock<Font> = OnceLock::new();
    FONT.get_or_init(|| {
        #[cfg(target_os = "ios")]
        eprintln!(
            "[RustRenderer] loading bundled UI font LilitaOne-Regular.ttf ({} bytes)",
            UI_FONT_BYTES.len()
        );
        Font::from_bytes(UI_FONT_BYTES, FontSettings::default())
            .expect("bundled Lilita One font should be valid")
    })
}

pub(super) struct UiAtlasGlyph {
    pub(super) character: char,
    pub(super) x: u32,
    pub(super) metrics: fontdue::Metrics,
    pub(super) bitmap: Vec<u8>,
}

pub(super) fn ui_atlas_glyphs() -> &'static [UiAtlasGlyph] {
    static GLYPHS: OnceLock<Vec<UiAtlasGlyph>> = OnceLock::new();
    GLYPHS.get_or_init(|| {
        let mut x = UI_ATLAS_PADDING;
        let glyphs = (32_u8..=126)
            .map(|byte| {
                let character = char::from(byte);
                let (metrics, bitmap) = ui_font().rasterize(character, UI_FONT_ATLAS_SIZE);
                let glyph = UiAtlasGlyph {
                    character,
                    x,
                    metrics,
                    bitmap,
                };
                x += glyph.metrics.width as u32 + UI_ATLAS_PADDING * 2;
                glyph
            })
            .collect::<Vec<_>>();
        assert!(x <= UI_ATLAS_WIDTH, "UI glyph atlas exceeds its width");
        glyphs
    })
}

#[cfg(target_os = "ios")]
static LAST_UI_DRAW_VERTEX_COUNT: AtomicUsize = AtomicUsize::new(usize::MAX);

#[cfg(target_os = "ios")]
fn log_ui_draw_vertex_count(count: usize) {
    if LAST_UI_DRAW_VERTEX_COUNT.swap(count, Ordering::Relaxed) != count {
        eprintln!("[RustRenderer] UI draw vertices={count}");
    }
}
