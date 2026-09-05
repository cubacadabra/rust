const MAX_UI_NODES: usize = 512;
const MAX_UI_DEPTH: usize = 32;
const MIN_TOUCH_TARGET: f32 = 44.0;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiInsets {
    #[serde(default)]
    pub(crate) top: f32,
    #[serde(default)]
    pub(crate) right: f32,
    #[serde(default)]
    pub(crate) bottom: f32,
    #[serde(default)]
    pub(crate) left: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct UiViewport {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) scale: f32,
    pub(crate) safe_area: UiInsets,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct UiRect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl UiRect {
    fn inset(self, amount: f32) -> Self {
        let amount = amount.max(0.0);
        Self {
            x: self.x + amount,
            y: self.y + amount,
            width: (self.width - amount * 2.0).max(0.0),
            height: (self.height - amount * 2.0).max(0.0),
        }
    }

    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x <= self.x + self.width && y <= self.y + self.height
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiDocument {
    #[serde(default)]
    pub(crate) nodes: Vec<UiNode>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UiNodeKind {
    #[default]
    Panel,
    Stack,
    Text,
    Button,
    Menu,
    Modal,
    Toggle,
    Slider,
    Joystick,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UiRegion {
    #[default]
    Canvas,
    Header,
    BottomCenter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiImage {
    Logo,
    Cube,
    Chat,
    Voice,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UiAnchor {
    #[default]
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UiDirection {
    #[default]
    Column,
    Row,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UiAlignment {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UiJustification {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub(crate) enum UiLength {
    Points(f32),
    Named(String),
}

impl Default for UiLength {
    fn default() -> Self {
        Self::Named("auto".to_owned())
    }
}

impl UiLength {
    fn is_fill(&self) -> bool {
        matches!(self, Self::Named(value) if value.eq_ignore_ascii_case("fill"))
    }

    fn resolve(&self, available: f32, intrinsic: f32) -> f32 {
        match self {
            Self::Points(value) => value.max(0.0),
            Self::Named(value) if value.eq_ignore_ascii_case("fill") => available.max(0.0),
            Self::Named(value) if value.ends_with('%') => value
                .trim_end_matches('%')
                .parse::<f32>()
                .map_or(intrinsic, |percent| {
                    available * (percent / 100.0).clamp(0.0, 1.0)
                }),
            Self::Named(_) => intrinsic,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiLayout {
    #[serde(default)]
    pub(crate) width: UiLength,
    #[serde(default)]
    pub(crate) height: UiLength,
    #[serde(default)]
    pub(crate) max_width: Option<f32>,
    #[serde(default)]
    pub(crate) max_height: Option<f32>,
    #[serde(default)]
    pub(crate) anchor: UiAnchor,
    #[serde(default)]
    pub(crate) direction: UiDirection,
    #[serde(default)]
    pub(crate) align: UiAlignment,
    #[serde(default)]
    pub(crate) justify: UiJustification,
    #[serde(default)]
    pub(crate) padding: f32,
    #[serde(default)]
    pub(crate) gap: f32,
    #[serde(default)]
    pub(crate) offset: [f32; 2],
    #[serde(default)]
    pub(crate) ignore_safe_area: bool,
    #[serde(default)]
    pub(crate) region: UiRegion,
}

fn default_foreground() -> String {
    "#ffffff".to_owned()
}

fn default_font_size() -> f32 {
    14.0
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiStyle {
    #[serde(default)]
    pub(crate) background: Option<String>,
    #[serde(default = "default_foreground")]
    pub(crate) foreground: String,
    #[serde(default)]
    pub(crate) border_color: Option<String>,
    #[serde(default)]
    pub(crate) border_width: f32,
    #[serde(default)]
    pub(crate) corner_radius: f32,
    #[serde(default = "default_font_size")]
    pub(crate) font_size: f32,
    #[serde(default)]
    pub(crate) text_align: UiAlignment,
    #[serde(default)]
    pub(crate) accent: Option<String>,
}

impl Default for UiStyle {
    fn default() -> Self {
        Self {
            background: None,
            foreground: default_foreground(),
            border_color: None,
            border_width: 0.0,
            corner_radius: 0.0,
            font_size: default_font_size(),
            text_align: UiAlignment::Start,
            accent: None,
        }
    }
}

fn default_visible() -> bool {
    true
}

fn default_maximum() -> f32 {
    1.0
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiNode {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) kind: UiNodeKind,
    #[serde(default)]
    pub(crate) text: String,
    #[serde(default)]
    pub(crate) icon: Option<String>,
    #[serde(default)]
    pub(crate) action: String,
    #[serde(default)]
    pub(crate) layout: UiLayout,
    #[serde(default)]
    pub(crate) style: UiStyle,
    #[serde(default)]
    pub(crate) children: Vec<UiNode>,
    #[serde(default = "default_visible")]
    pub(crate) visible: bool,
    #[serde(default)]
    pub(crate) visible_in: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) disabled: bool,
    #[serde(default)]
    pub(crate) blocks_input: bool,
    #[serde(default)]
    pub(crate) checked: bool,
    #[serde(default)]
    pub(crate) value: f32,
    #[serde(default)]
    pub(crate) minimum: f32,
    #[serde(default = "default_maximum")]
    pub(crate) maximum: f32,
    #[serde(default)]
    pub(crate) value_x: f32,
    #[serde(default)]
    pub(crate) value_y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiRenderNode {
    pub(crate) id: String,
    pub(crate) kind: UiNodeKind,
    pub(crate) rect: UiRect,
    pub(crate) text: String,
    pub(crate) icon: Option<String>,
    pub(crate) background: Option<[f32; 4]>,
    pub(crate) foreground: [f32; 4],
    pub(crate) border_color: Option<[f32; 4]>,
    pub(crate) border_width: f32,
    pub(crate) corner_radius: f32,
    pub(crate) font_size: f32,
    pub(crate) text_align: UiAlignment,
    pub(crate) accent: [f32; 4],
    pub(crate) image: Option<UiImage>,
    pub(crate) image_invert: bool,
    pub(crate) value: f32,
    pub(crate) value_x: f32,
    pub(crate) value_y: f32,
    pub(crate) checked: bool,
    pub(crate) pressed: bool,
    pub(crate) disabled: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct UiFrame {
    pub(crate) viewport: UiViewport,
    pub(crate) nodes: Vec<UiRenderNode>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiEvent {
    pub(crate) node_id: String,
    pub(crate) action: String,
    pub(crate) phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) value: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) y: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiPointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Clone, Debug)]
struct UiHitRegion {
    id: String,
    action: String,
    kind: UiNodeKind,
    rect: UiRect,
    disabled: bool,
}

#[derive(Clone, Debug)]
struct UiCapture {
    region: UiHitRegion,
    joystick_origin: Option<(f32, f32)>,
}

pub(crate) struct UiRuntime {
    document: UiDocument,
    world_id: String,
    viewport: UiViewport,
    frame: UiFrame,
    hit_regions: Vec<UiHitRegion>,
    captures: HashMap<u64, UiCapture>,
    host_events: VecDeque<UiEvent>,
    script_events: VecDeque<UiEvent>,
    event_buffer: Vec<u8>,
    dirty: bool,
    shared_authenticated: bool,
    shared_modal_progress: f32,
    shared_modal_target: f32,
    shared_modal_tab: usize,
    joystick_gesture_rect: UiRect,
}
