use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

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
    shared_modal_progress: f32,
    shared_modal_target: f32,
    shared_modal_tab: usize,
}

impl Default for UiRuntime {
    fn default() -> Self {
        Self {
            document: UiDocument::default(),
            world_id: "lobby".to_owned(),
            viewport: UiViewport::default(),
            frame: UiFrame::default(),
            hit_regions: Vec::new(),
            captures: HashMap::new(),
            host_events: VecDeque::new(),
            script_events: VecDeque::new(),
            event_buffer: Vec::new(),
            dirty: false,
            shared_modal_progress: 0.0,
            shared_modal_target: 0.0,
            shared_modal_tab: 0,
        }
    }
}

impl UiRuntime {
    pub(crate) fn document_node_count(&self) -> usize {
        fn count(nodes: &[UiNode]) -> usize {
            nodes.iter().map(|node| 1 + count(&node.children)).sum()
        }

        count(&self.document.nodes)
    }

    pub(crate) fn set_viewport(&mut self, viewport: UiViewport) {
        let viewport = UiViewport {
            width: viewport.width.max(0.0),
            height: viewport.height.max(0.0),
            scale: viewport.scale.max(0.1),
            safe_area: UiInsets {
                top: viewport.safe_area.top.max(0.0),
                right: viewport.safe_area.right.max(0.0),
                bottom: viewport.safe_area.bottom.max(0.0),
                left: viewport.safe_area.left.max(0.0),
            },
        };
        if self.viewport != viewport {
            self.viewport = viewport;
            self.dirty = true;
        }
    }

    pub(crate) fn set_world_id(&mut self, world_id: &str) {
        if self.world_id != world_id {
            self.world_id = world_id.to_owned();
            self.captures.clear();
            self.dirty = true;
        }
    }

    pub(crate) fn set_document_json(&mut self, source: &str) -> Result<(), String> {
        let document: UiDocument =
            serde_json::from_str(source).map_err(|error| error.to_string())?;
        validate_document(&document)?;
        self.document = document;
        self.captures.clear();
        self.dirty = true;
        Ok(())
    }

    pub(crate) fn clear(&mut self) {
        self.document.nodes.clear();
        self.captures.clear();
        self.dirty = true;
    }

    pub(crate) fn set_text(&mut self, id: &str, text: &str) -> bool {
        let Some(node) = find_node_mut(&mut self.document.nodes, id) else {
            return false;
        };
        node.text = text.chars().take(256).collect();
        self.dirty = true;
        true
    }

    pub(crate) fn set_value(&mut self, id: &str, value: f32) -> bool {
        let Some(node) = find_node_mut(&mut self.document.nodes, id) else {
            return false;
        };
        node.value = value.clamp(node.minimum, node.maximum.max(node.minimum));
        self.dirty = true;
        true
    }

    pub(crate) fn set_checked(&mut self, id: &str, checked: bool) -> bool {
        let Some(node) = find_node_mut(&mut self.document.nodes, id) else {
            return false;
        };
        node.checked = checked;
        node.value = f32::from(checked);
        self.dirty = true;
        true
    }

    pub(crate) fn set_visible(&mut self, id: &str, visible: bool) -> bool {
        let Some(node) = find_node_mut(&mut self.document.nodes, id) else {
            return false;
        };
        node.visible = visible;
        self.dirty = true;
        true
    }

    pub(crate) fn frame(&mut self) -> &UiFrame {
        self.rebuild_if_needed();
        &self.frame
    }

    pub(crate) fn is_interactive_at(&mut self, x: f32, y: f32) -> bool {
        self.rebuild_if_needed();
        self.hit_regions.iter().rev().any(|region| {
            !region.disabled
                && region.rect.contains(x, y)
                && (matches!(
                    region.kind,
                    UiNodeKind::Button
                        | UiNodeKind::Toggle
                        | UiNodeKind::Slider
                        | UiNodeKind::Joystick
                ) || !region.action.is_empty())
        })
    }

    pub(crate) fn advance(&mut self, delta: f32) {
        if (self.shared_modal_progress - self.shared_modal_target).abs() <= f32::EPSILON {
            return;
        }
        let step = (delta.max(0.0) / SHARED_MODAL_ANIMATION_SECONDS).clamp(0.0, 1.0);
        self.shared_modal_progress +=
            (self.shared_modal_target - self.shared_modal_progress) * step;
        self.shared_modal_progress = self.shared_modal_progress.clamp(0.0, 1.0);
        if (self.shared_modal_progress - self.shared_modal_target).abs() < 0.001 {
            self.shared_modal_progress = self.shared_modal_target;
        }
        self.dirty = true;
    }

    pub(crate) fn pointer(
        &mut self,
        pointer_id: u64,
        phase: UiPointerPhase,
        x: f32,
        y: f32,
    ) -> bool {
        self.rebuild_if_needed();
        match phase {
            UiPointerPhase::Down => {
                let Some(region) = self
                    .hit_regions
                    .iter()
                    .rev()
                    .find(|region| !region.disabled && region.rect.contains(x, y))
                    .cloned()
                else {
                    return false;
                };
                self.captures.insert(pointer_id, UiCapture { region });
                self.update_pointer_control(pointer_id, x, y, "change");
                self.dirty = true;
                true
            }
            UiPointerPhase::Move => {
                if !self.captures.contains_key(&pointer_id) {
                    return false;
                }
                self.update_pointer_control(pointer_id, x, y, "change");
                self.dirty = true;
                true
            }
            UiPointerPhase::Up => {
                let Some(capture) = self.captures.remove(&pointer_id) else {
                    return false;
                };
                if capture.region.kind == UiNodeKind::Joystick {
                    self.reset_joystick(&capture.region, "release");
                } else if capture.region.rect.contains(x, y) {
                    self.activate(capture.region, x);
                }
                self.dirty = true;
                true
            }
            UiPointerPhase::Cancel => {
                let capture = self.captures.remove(&pointer_id);
                if let Some(capture) = &capture
                    && capture.region.kind == UiNodeKind::Joystick
                {
                    self.reset_joystick(&capture.region, "cancel");
                }
                let consumed = capture.is_some();
                self.dirty |= consumed;
                consumed
            }
        }
    }

    pub(crate) fn take_script_events(&mut self) -> Vec<UiEvent> {
        self.script_events.drain(..).collect()
    }

    pub(crate) fn poll_event(&mut self) -> bool {
        let Some(event) = self.host_events.pop_front() else {
            self.event_buffer.clear();
            return false;
        };
        self.event_buffer = serde_json::to_vec(&event).unwrap_or_default();
        true
    }

    pub(crate) fn event_buffer(&self) -> &[u8] {
        &self.event_buffer
    }

    fn rebuild_if_needed(&mut self) {
        if !self.dirty {
            return;
        }
        let safe = UiRect {
            x: self.viewport.safe_area.left,
            y: self.viewport.safe_area.top,
            width: (self.viewport.width
                - self.viewport.safe_area.left
                - self.viewport.safe_area.right)
                .max(0.0),
            height: (self.viewport.height
                - self.viewport.safe_area.top
                - self.viewport.safe_area.bottom)
                .max(0.0),
        };
        let pressed = self
            .captures
            .values()
            .map(|capture| capture.region.id.as_str())
            .collect::<HashSet<_>>();
        let mut nodes = Vec::new();
        let mut hit_regions = Vec::new();
        let mut overlay_roots = Vec::new();
        for node in &self.document.nodes {
            // Movement controls belong to the engine-owned HUD layer. Keep
            // them out of the normal document pass so a world-scoped or
            // script-hidden document node cannot remove them accidentally.
            if is_persistent_gameplay_control(node) {
                continue;
            }
            if !node_is_visible(node, &self.world_id) {
                continue;
            }
            if node.layout.region != UiRegion::Canvas {
                continue;
            }
            if matches!(node.kind, UiNodeKind::Menu | UiNodeKind::Modal) {
                overlay_roots.push(node);
                continue;
            }
            let available = if node.layout.ignore_safe_area {
                UiRect {
                    x: 0.0,
                    y: 0.0,
                    width: self.viewport.width,
                    height: self.viewport.height,
                }
            } else {
                safe
            };
            let intrinsic = measure_node(node, available.width, available.height, &self.world_id);
            let width = clamp_length(
                node.layout.width.resolve(available.width, intrinsic.0),
                node.layout.max_width,
                available.width,
            );
            let height = clamp_length(
                node.layout.height.resolve(available.height, intrinsic.1),
                node.layout.max_height,
                available.height,
            );
            let rect = anchored_rect(
                available,
                width,
                height,
                node.layout.anchor,
                node.layout.offset,
            );
            layout_node(
                node,
                rect,
                &self.world_id,
                &pressed,
                &mut nodes,
                &mut hit_regions,
            );
        }
        // Draw the platform-owned controls last so game UI cannot cover them.
        // Their surfaces consume taps without emitting events until platform
        // actions are connected.
        let shared_header = shared_header_nodes(self.viewport, safe);
        for node in shared_header
            .nodes
            .iter()
            .filter(|node| node.id.ends_with("_surface"))
        {
            hit_regions.push(UiHitRegion {
                id: node.id.clone(),
                action: (node.id == "__shared_header_logo_surface")
                    .then(|| "shared.header.toggle".to_owned())
                    .unwrap_or_default(),
                kind: UiNodeKind::Panel,
                rect: node.rect,
                disabled: false,
            });
        }
        nodes.extend(shared_header.nodes);

        let header_nodes = self
            .document
            .nodes
            .iter()
            .filter(|node| {
                node_is_visible(node, &self.world_id) && node.layout.region == UiRegion::Header
            })
            .collect::<Vec<_>>();
        layout_region_roots(
            &header_nodes,
            UiRect {
                x: shared_header.custom_x,
                y: shared_header.y,
                width: (safe.x + safe.width - SHARED_HEADER_MARGIN - shared_header.custom_x)
                    .max(0.0),
                height: shared_header.size,
            },
            false,
            &self.world_id,
            &pressed,
            &mut nodes,
            &mut hit_regions,
        );

        let bottom_nodes = self
            .document
            .nodes
            .iter()
            .filter(|node| {
                node_is_visible(node, &self.world_id)
                    && node.layout.region == UiRegion::BottomCenter
            })
            .collect::<Vec<_>>();
        layout_region_roots(
            &bottom_nodes,
            UiRect {
                x: safe.x + SHARED_HEADER_MARGIN,
                y: safe.y + safe.height - SHARED_HEADER_MARGIN - REGION_CONTROL_HEIGHT,
                width: (safe.width - SHARED_HEADER_MARGIN * 2.0).max(0.0),
                height: REGION_CONTROL_HEIGHT,
            },
            true,
            &self.world_id,
            &pressed,
            &mut nodes,
            &mut hit_regions,
        );

        // Keep movement controls alongside the shared header: they are
        // available in every world and remain above ordinary game UI. Clone
        // the document definitions so the game package still owns their
        // styling and actions, but deliberately ignore document visibility
        // and world scope for this engine-owned layer.
        for node in &self.document.nodes {
            if !is_persistent_gameplay_control(node) {
                continue;
            }
            let mut persistent_node = node.clone();
            persistent_node.visible = true;
            persistent_node.visible_in = None;
            let intrinsic = measure_node(&persistent_node, safe.width, safe.height, &self.world_id);
            let width = clamp_length(
                persistent_node
                    .layout
                    .width
                    .resolve(safe.width, intrinsic.0),
                persistent_node.layout.max_width,
                safe.width,
            );
            let height = clamp_length(
                persistent_node
                    .layout
                    .height
                    .resolve(safe.height, intrinsic.1),
                persistent_node.layout.max_height,
                safe.height,
            );
            let rect = anchored_rect(
                safe,
                width,
                height,
                persistent_node.layout.anchor,
                persistent_node.layout.offset,
            );
            layout_node(
                &persistent_node,
                rect,
                &self.world_id,
                &pressed,
                &mut nodes,
                &mut hit_regions,
            );
        }

        // Menus and modals are a deliberate top layer. This keeps a full-screen
        // scrim above the shared header and touch controls while its menu
        // children remain above the scrim itself.
        for node in overlay_roots {
            let available = if node.layout.ignore_safe_area {
                UiRect {
                    x: 0.0,
                    y: 0.0,
                    width: self.viewport.width,
                    height: self.viewport.height,
                }
            } else {
                safe
            };
            let intrinsic = measure_node(node, available.width, available.height, &self.world_id);
            let width = clamp_length(
                node.layout.width.resolve(available.width, intrinsic.0),
                node.layout.max_width,
                available.width,
            );
            let height = clamp_length(
                node.layout.height.resolve(available.height, intrinsic.1),
                node.layout.max_height,
                available.height,
            );
            let rect = anchored_rect(
                available,
                width,
                height,
                node.layout.anchor,
                node.layout.offset,
            );
            layout_node(
                node,
                rect,
                &self.world_id,
                &pressed,
                &mut nodes,
                &mut hit_regions,
            );
        }
        let modal = shared_modal_nodes(
            self.viewport,
            safe,
            self.shared_modal_progress,
            self.shared_modal_target,
            self.shared_modal_tab,
        );
        nodes.extend(modal.nodes);
        hit_regions.extend(modal.hit_regions);
        self.frame = UiFrame {
            viewport: self.viewport,
            nodes,
        };
        self.hit_regions = hit_regions;
        self.dirty = false;
    }

    fn update_pointer_control(&mut self, pointer_id: u64, x: f32, y: f32, phase: &str) {
        let Some(capture) = self.captures.get(&pointer_id).cloned() else {
            return;
        };
        match capture.region.kind {
            UiNodeKind::Slider => self.update_slider(&capture.region, x, phase),
            UiNodeKind::Joystick => self.update_joystick(&capture.region, x, y, phase),
            _ => {}
        }
    }

    fn update_slider(&mut self, region: &UiHitRegion, x: f32, phase: &str) {
        let fraction = ((x - region.rect.x) / region.rect.width.max(1.0)).clamp(0.0, 1.0);
        let Some(node) = find_node_mut(&mut self.document.nodes, &region.id) else {
            return;
        };
        let value = node.minimum + fraction * (node.maximum - node.minimum).max(0.0);
        if (node.value - value).abs() <= f32::EPSILON {
            return;
        }
        node.value = value;
        self.push_event(UiEvent {
            node_id: region.id.clone(),
            action: region.action.clone(),
            phase: phase.to_owned(),
            value: Some(value),
            x: None,
            y: None,
        });
    }

    fn update_joystick(&mut self, region: &UiHitRegion, x: f32, y: f32, phase: &str) {
        let radius = region.rect.width.min(region.rect.height).max(1.0) * 0.5;
        let center_x = region.rect.x + region.rect.width * 0.5;
        let center_y = region.rect.y + region.rect.height * 0.5;
        let mut value_x = (x - center_x) / radius;
        let mut value_y = (y - center_y) / radius;
        let length = value_x.hypot(value_y);
        if length > 1.0 {
            value_x /= length;
            value_y /= length;
        }
        let Some(node) = find_node_mut(&mut self.document.nodes, &region.id) else {
            return;
        };
        if (node.value_x - value_x).abs() <= f32::EPSILON
            && (node.value_y - value_y).abs() <= f32::EPSILON
        {
            return;
        }
        node.value_x = value_x;
        node.value_y = value_y;
        self.push_event(UiEvent {
            node_id: region.id.clone(),
            action: region.action.clone(),
            phase: phase.to_owned(),
            value: None,
            x: Some(value_x),
            y: Some(value_y),
        });
    }

    fn reset_joystick(&mut self, region: &UiHitRegion, phase: &str) {
        let Some(node) = find_node_mut(&mut self.document.nodes, &region.id) else {
            return;
        };
        node.value_x = 0.0;
        node.value_y = 0.0;
        self.push_event(UiEvent {
            node_id: region.id.clone(),
            action: region.action.clone(),
            phase: phase.to_owned(),
            value: None,
            x: Some(0.0),
            y: Some(0.0),
        });
    }

    fn activate(&mut self, region: UiHitRegion, x: f32) {
        if region.id == "__shared_header_logo_surface" {
            self.shared_modal_target = if self.shared_modal_target > 0.5 {
                0.0
            } else {
                1.0
            };
            self.dirty = true;
            return;
        }
        if region.id == "__shared_modal_scrim" {
            self.shared_modal_target = 0.0;
            self.dirty = true;
            return;
        }
        if let Some(tab) = shared_modal_tab_index(&region.id) {
            self.shared_modal_tab = tab;
            self.dirty = true;
            return;
        }
        match region.kind {
            UiNodeKind::Toggle => {
                let Some(node) = find_node_mut(&mut self.document.nodes, &region.id) else {
                    return;
                };
                node.checked = !node.checked;
                node.value = f32::from(node.checked);
                let value = node.value;
                self.push_event(UiEvent {
                    node_id: region.id,
                    action: region.action,
                    phase: "activate".to_owned(),
                    value: Some(value),
                    x: None,
                    y: None,
                });
            }
            UiNodeKind::Slider => self.update_slider_on_activation(region, x),
            UiNodeKind::Button => self.push_event(UiEvent {
                node_id: region.id,
                action: region.action,
                phase: "activate".to_owned(),
                value: None,
                x: None,
                y: None,
            }),
            _ if !region.action.is_empty() => self.push_event(UiEvent {
                node_id: region.id,
                action: region.action,
                phase: "activate".to_owned(),
                value: None,
                x: None,
                y: None,
            }),
            _ => {}
        }
    }

    fn update_slider_on_activation(&mut self, region: UiHitRegion, x: f32) {
        let fraction = ((x - region.rect.x) / region.rect.width.max(1.0)).clamp(0.0, 1.0);
        let Some(node) = find_node_mut(&mut self.document.nodes, &region.id) else {
            return;
        };
        node.value = node.minimum + fraction * (node.maximum - node.minimum).max(0.0);
        let value = node.value;
        self.push_event(UiEvent {
            node_id: region.id,
            action: region.action,
            phase: "commit".to_owned(),
            value: Some(value),
            x: None,
            y: None,
        });
    }

    fn push_event(&mut self, event: UiEvent) {
        const MAX_PENDING_EVENTS: usize = 128;
        if self.host_events.len() >= MAX_PENDING_EVENTS {
            self.host_events.pop_front();
        }
        if self.script_events.len() >= MAX_PENDING_EVENTS {
            self.script_events.pop_front();
        }
        self.host_events.push_back(event.clone());
        self.script_events.push_back(event);
        self.dirty = true;
    }
}

fn validate_document(document: &UiDocument) -> Result<(), String> {
    fn visit(
        nodes: &[UiNode],
        depth: usize,
        count: &mut usize,
        ids: &mut HashSet<String>,
    ) -> Result<(), String> {
        if depth > MAX_UI_DEPTH {
            return Err(format!("UI nesting exceeds {MAX_UI_DEPTH} levels"));
        }
        for node in nodes {
            *count += 1;
            if *count > MAX_UI_NODES {
                return Err(format!("UI document exceeds {MAX_UI_NODES} nodes"));
            }
            if node.id.trim().is_empty() {
                return Err("Every UI node needs a non-empty id".to_owned());
            }
            if !ids.insert(node.id.clone()) {
                return Err(format!("Duplicate UI node id: {}", node.id));
            }
            visit(&node.children, depth + 1, count, ids)?;
        }
        Ok(())
    }

    visit(&document.nodes, 0, &mut 0, &mut HashSet::new())
}

fn find_node_mut<'a>(nodes: &'a mut [UiNode], id: &str) -> Option<&'a mut UiNode> {
    for node in nodes {
        if node.id == id {
            return Some(node);
        }
        if let Some(found) = find_node_mut(&mut node.children, id) {
            return Some(found);
        }
    }
    None
}

fn node_is_visible(node: &UiNode, world_id: &str) -> bool {
    node.visible
        && node
            .visible_in
            .as_ref()
            .is_none_or(|worlds| worlds.iter().any(|world| world == world_id))
}

fn is_persistent_gameplay_control(node: &UiNode) -> bool {
    matches!(
        node.id.as_str(),
        "player-joystick" | "player-jump" | "player-run"
    )
}

fn measure_node(
    node: &UiNode,
    available_width: f32,
    available_height: f32,
    world_id: &str,
) -> (f32, f32) {
    let padding = node.layout.padding.max(0.0) * 2.0;
    let font_size = node.style.font_size.max(7.0);
    let text_width = node
        .text
        .lines()
        .map(|line| line.chars().count() as f32 * font_size * 0.72)
        .fold(0.0, f32::max);
    let text_height = node.text.lines().count().max(1) as f32 * font_size * 1.25;
    let child_sizes = node
        .children
        .iter()
        .filter(|child| node_is_visible(child, world_id))
        .map(|child| measure_node(child, available_width, available_height, world_id))
        .collect::<Vec<_>>();
    let gap = node.layout.gap.max(0.0) * child_sizes.len().saturating_sub(1) as f32;
    let (children_width, children_height) = match node.layout.direction {
        UiDirection::Row => (
            child_sizes.iter().map(|size| size.0).sum::<f32>() + gap,
            child_sizes.iter().map(|size| size.1).fold(0.0, f32::max),
        ),
        UiDirection::Column => (
            child_sizes.iter().map(|size| size.0).fold(0.0, f32::max),
            child_sizes.iter().map(|size| size.1).sum::<f32>() + gap,
        ),
    };
    let leaf_width = match node.kind {
        UiNodeKind::Toggle => 72.0,
        UiNodeKind::Slider => 180.0,
        UiNodeKind::Joystick => 120.0,
        UiNodeKind::Button if node.icon.is_some() && node.text.is_empty() => MIN_TOUCH_TARGET,
        UiNodeKind::Button => text_width.max(MIN_TOUCH_TARGET),
        _ => text_width,
    };
    let leaf_height = match node.kind {
        UiNodeKind::Joystick => 120.0,
        UiNodeKind::Button | UiNodeKind::Toggle | UiNodeKind::Slider => MIN_TOUCH_TARGET,
        _ => text_height,
    };
    let intrinsic_width = leaf_width.max(children_width) + padding;
    let intrinsic_height = if node.children.is_empty() {
        leaf_height + padding
    } else {
        children_height.max(if node.text.is_empty() {
            0.0
        } else {
            leaf_height
        }) + padding
    };
    let minimum = if matches!(
        node.kind,
        UiNodeKind::Button | UiNodeKind::Toggle | UiNodeKind::Slider | UiNodeKind::Joystick
    ) {
        MIN_TOUCH_TARGET
    } else {
        0.0
    };
    (
        clamp_length(
            node.layout
                .width
                .resolve(available_width, intrinsic_width)
                .max(minimum.min(available_width)),
            node.layout.max_width,
            available_width,
        ),
        clamp_length(
            node.layout
                .height
                .resolve(available_height, intrinsic_height)
                .max(minimum.min(available_height)),
            node.layout.max_height,
            available_height,
        ),
    )
}

fn clamp_length(value: f32, maximum: Option<f32>, available: f32) -> f32 {
    value
        .min(maximum.unwrap_or(f32::MAX).max(0.0))
        .min(available.max(0.0))
}

fn anchored_rect(
    parent: UiRect,
    width: f32,
    height: f32,
    anchor: UiAnchor,
    offset: [f32; 2],
) -> UiRect {
    let horizontal = match anchor {
        UiAnchor::Top | UiAnchor::Center | UiAnchor::Bottom => 0.5,
        UiAnchor::TopRight | UiAnchor::Right | UiAnchor::BottomRight => 1.0,
        _ => 0.0,
    };
    let vertical = match anchor {
        UiAnchor::Left | UiAnchor::Center | UiAnchor::Right => 0.5,
        UiAnchor::BottomLeft | UiAnchor::Bottom | UiAnchor::BottomRight => 1.0,
        _ => 0.0,
    };
    let x = parent.x + (parent.width - width) * horizontal + offset[0];
    let y = parent.y + (parent.height - height) * vertical + offset[1];
    UiRect {
        x: x.clamp(parent.x, (parent.x + parent.width - width).max(parent.x)),
        y: y.clamp(parent.y, (parent.y + parent.height - height).max(parent.y)),
        width,
        height,
    }
}

const SHARED_MODAL_ANIMATION_SECONDS: f32 = 0.22;
const SHARED_HEADER_SIZE: f32 = 56.0;
const SHARED_HEADER_MARGIN: f32 = 24.0;
const SHARED_HEADER_GAP: f32 = 12.0;
const SHARED_HEADER_CELL_GAP: f32 = 8.0;
const SHARED_HEADER_PILL_PADDING: f32 = 14.0;
const REGION_CONTROL_HEIGHT: f32 = 68.0;

struct SharedHeaderGeometry {
    nodes: Vec<UiRenderNode>,
    custom_x: f32,
    y: f32,
    size: f32,
}

fn shared_header_nodes(viewport: UiViewport, safe: UiRect) -> SharedHeaderGeometry {
    if viewport.width <= 0.0 || viewport.height <= 0.0 {
        return SharedHeaderGeometry {
            nodes: Vec::new(),
            custom_x: safe.x,
            y: safe.y,
            size: 0.0,
        };
    }

    // Safe-area insets make a 4:3 iPad portrait window slightly shorter than
    // its raw bounds, so use a more forgiving threshold here.
    let portraitish = safe.height > safe.width * 1.15;
    let compact = safe.width < 600.0 || portraitish;
    let margin = if compact { 12.0 } else { SHARED_HEADER_MARGIN };
    let group_gap = if compact { 8.0 } else { SHARED_HEADER_GAP };
    let cell_gap = if compact { 6.0 } else { SHARED_HEADER_CELL_GAP };
    let pill_padding = if compact {
        8.0
    } else {
        SHARED_HEADER_PILL_PADDING
    };
    let maximum_size = if compact { 44.0 } else { SHARED_HEADER_SIZE };
    let x = safe.x + margin;
    let y = safe.y + margin;
    let available_width = (safe.width - margin * 2.0).max(0.0);
    let size = maximum_size
        .min(((available_width - group_gap - cell_gap * 2.0 - pill_padding * 2.0) / 4.0).max(0.0));
    if size < 1.0 {
        return SharedHeaderGeometry {
            nodes: Vec::new(),
            custom_x: x,
            y,
            size: 0.0,
        };
    }

    let surface = [0.02, 0.04, 0.05, 0.92];
    let logo_rect = UiRect {
        x,
        y,
        width: size,
        height: size,
    };
    let controls_x = x + size + group_gap;
    let cell_size = size;
    let controls_width = cell_size * 3.0 + cell_gap * 2.0 + pill_padding * 2.0;
    let icon_size = (cell_size - 16.0).max(1.0);

    let mut nodes = vec![header_surface(
        "__shared_header_logo_surface",
        logo_rect,
        size * 0.5,
        surface,
    )];
    nodes.push(header_image(
        "__shared_header_logo",
        UiImage::Logo,
        logo_rect.inset((cell_size - icon_size) * 0.5),
        false,
    ));

    let controls_rect = UiRect {
        x: controls_x,
        y,
        width: controls_width,
        height: size,
    };
    nodes.push(header_surface(
        "__shared_header_controls_surface",
        controls_rect,
        size * 0.5,
        surface,
    ));
    for (index, image) in [UiImage::Cube, UiImage::Chat, UiImage::Voice]
        .into_iter()
        .enumerate()
    {
        let cell = UiRect {
            x: controls_x + pill_padding + index as f32 * (cell_size + cell_gap),
            y,
            width: cell_size,
            height: size,
        };
        nodes.push(header_image(
            match image {
                UiImage::Cube => "__shared_header_cube",
                UiImage::Chat => "__shared_header_chat",
                UiImage::Voice => "__shared_header_voice",
                UiImage::Logo => unreachable!(),
            },
            image,
            cell.inset((cell_size - icon_size) * 0.5),
            true,
        ));
    }
    SharedHeaderGeometry {
        nodes,
        custom_x: controls_rect.x + controls_rect.width + group_gap,
        y,
        size,
    }
}

struct SharedModalGeometry {
    nodes: Vec<UiRenderNode>,
    hit_regions: Vec<UiHitRegion>,
}

fn shared_modal_nodes(
    viewport: UiViewport,
    safe: UiRect,
    progress: f32,
    target: f32,
    selected_tab: usize,
) -> SharedModalGeometry {
    if viewport.width <= 0.0
        || viewport.height <= 0.0
        || (progress <= 0.0 && target <= 0.0)
    {
        return SharedModalGeometry {
            nodes: Vec::new(),
            hit_regions: Vec::new(),
        };
    }

    let compact = safe.width < 600.0 || safe.height > safe.width * 1.15;
    let horizontal_margin = if compact { 12.0 } else { 18.0 };
    let top_gap = if compact { 48.0 } else { 52.0 };
    let panel = UiRect {
        x: (safe.x + horizontal_margin).min(viewport.width),
        y: (safe.y + top_gap).min(viewport.height),
        width: (safe.width - horizontal_margin * 2.0).max(0.0),
        height: (safe.height - top_gap).max(0.0),
    };
    let eased = progress * progress * (3.0 - 2.0 * progress);
    let slide_distance = (viewport.height - panel.y).max(0.0);
    let animated_panel = UiRect {
        y: panel.y + slide_distance * (1.0 - eased),
        ..panel
    };
    let scrim = UiRect {
        x: 0.0,
        y: 0.0,
        width: viewport.width,
        height: viewport.height,
    };
    let mut nodes = vec![modal_node(
        "__shared_modal_scrim",
        UiNodeKind::Modal,
        scrim,
        Some([0.01, 0.015, 0.02, 0.54 * progress]),
        None,
        0.0,
    )];
    let mut hit_regions = vec![UiHitRegion {
        id: "__shared_modal_scrim".to_owned(),
        action: "shared.modal.close".to_owned(),
        kind: UiNodeKind::Modal,
        rect: scrim,
        disabled: false,
    }];

    nodes.push(modal_node(
        "__shared_modal_panel",
        UiNodeKind::Panel,
        animated_panel,
        Some([0.10, 0.12, 0.15, 0.94]),
        Some([0.52, 0.60, 0.64, 0.32]),
        if compact { 18.0 } else { 22.0 },
    ));
    hit_regions.push(UiHitRegion {
        id: "__shared_modal_panel".to_owned(),
        action: String::new(),
        kind: UiNodeKind::Panel,
        rect: animated_panel,
        disabled: false,
    });

    let tab_padding = if compact { 12.0 } else { 18.0 };
    let tab_gap = if compact { 5.0 } else { 8.0 };
    let tab_row = UiRect {
        x: animated_panel.x + tab_padding,
        y: animated_panel.y + tab_padding,
        width: (animated_panel.width - tab_padding * 2.0).max(0.0),
        height: if compact { 46.0 } else { 50.0 },
    };
    let tab_width = ((tab_row.width - tab_gap * 4.0) / 5.0).max(0.0);
    for (index, label) in ["About", "Settings", "People", "Report", "Help"]
        .into_iter()
        .enumerate()
    {
        let tab_rect = UiRect {
            x: tab_row.x + index as f32 * (tab_width + tab_gap),
            y: tab_row.y,
            width: tab_width,
            height: tab_row.height,
        };
        let selected = index == selected_tab;
        let id = format!("__shared_modal_tab_{index}");
        nodes.push(modal_node(
            &id,
            UiNodeKind::Button,
            tab_rect,
            Some(if selected {
                [0.24, 0.30, 0.34, 0.98]
            } else {
                [0.13, 0.16, 0.19, 0.76]
            }),
            Some(if selected {
                [0.78, 0.87, 0.88, 0.72]
            } else {
                [0.50, 0.59, 0.62, 0.24]
            }),
            if compact { 12.0 } else { 14.0 },
        ));
        if let Some(node) = nodes.last_mut() {
            node.text = label.to_owned();
            node.font_size = if compact { 12.0 } else { 14.0 };
            node.text_align = UiAlignment::Center;
        }
        hit_regions.push(UiHitRegion {
            id,
            action: "shared.modal.tab".to_owned(),
            kind: UiNodeKind::Button,
            rect: tab_rect,
            disabled: false,
        });
    }

    // The body intentionally has no copy or controls from the reference
    // image. It is simply a quiet surface reserved for future tab content.
    let body = UiRect {
        x: animated_panel.x + tab_padding,
        y: tab_row.y + tab_row.height + tab_padding,
        width: (animated_panel.width - tab_padding * 2.0).max(0.0),
        height: (animated_panel.height - tab_padding * 3.0 - tab_row.height).max(0.0),
    };
    nodes.push(modal_node(
        "__shared_modal_body",
        UiNodeKind::Panel,
        body,
        Some([0.07, 0.09, 0.11, 0.52]),
        Some([0.42, 0.50, 0.54, 0.18]),
        if compact { 12.0 } else { 16.0 },
    ));
    hit_regions.push(UiHitRegion {
        id: "__shared_modal_body".to_owned(),
        action: String::new(),
        kind: UiNodeKind::Panel,
        rect: body,
        disabled: false,
    });

    SharedModalGeometry { nodes, hit_regions }
}

fn modal_node(
    id: &str,
    kind: UiNodeKind,
    rect: UiRect,
    background: Option<[f32; 4]>,
    border_color: Option<[f32; 4]>,
    corner_radius: f32,
) -> UiRenderNode {
    UiRenderNode {
        id: id.to_owned(),
        kind,
        rect,
        text: String::new(),
        icon: None,
        background,
        foreground: [0.95, 0.97, 0.96, 1.0],
        border_color,
        border_width: if border_color.is_some() { 1.0 } else { 0.0 },
        corner_radius,
        font_size: default_font_size(),
        text_align: UiAlignment::Start,
        accent: [0.10, 0.55, 0.92, 1.0],
        image: None,
        image_invert: false,
        value: 0.0,
        value_x: 0.0,
        value_y: 0.0,
        checked: false,
        pressed: false,
        disabled: false,
    }
}

fn shared_modal_tab_index(id: &str) -> Option<usize> {
    id.strip_prefix("__shared_modal_tab_")?.parse().ok()
}

fn header_surface(
    id: &str,
    rect: UiRect,
    corner_radius: f32,
    background: [f32; 4],
) -> UiRenderNode {
    UiRenderNode {
        id: id.to_owned(),
        kind: UiNodeKind::Panel,
        rect,
        text: String::new(),
        icon: None,
        background: Some(background),
        foreground: [1.0; 4],
        border_color: None,
        border_width: 0.0,
        corner_radius,
        font_size: default_font_size(),
        text_align: UiAlignment::Start,
        accent: [0.0, 0.58, 1.0, 1.0],
        image: None,
        image_invert: false,
        value: 0.0,
        value_x: 0.0,
        value_y: 0.0,
        checked: false,
        pressed: false,
        disabled: false,
    }
}

fn header_image(id: &str, image: UiImage, rect: UiRect, image_invert: bool) -> UiRenderNode {
    UiRenderNode {
        id: id.to_owned(),
        kind: UiNodeKind::Panel,
        rect,
        text: String::new(),
        icon: None,
        background: None,
        foreground: [1.0; 4],
        border_color: None,
        border_width: 0.0,
        corner_radius: 0.0,
        font_size: default_font_size(),
        text_align: UiAlignment::Start,
        accent: [0.0, 0.58, 1.0, 1.0],
        image: Some(image),
        image_invert,
        value: 0.0,
        value_x: 0.0,
        value_y: 0.0,
        checked: false,
        pressed: false,
        disabled: false,
    }
}

fn layout_region_roots(
    roots: &[&UiNode],
    parent: UiRect,
    centered: bool,
    world_id: &str,
    pressed: &HashSet<&str>,
    nodes: &mut Vec<UiRenderNode>,
    hit_regions: &mut Vec<UiHitRegion>,
) {
    if roots.is_empty() || parent.width <= 0.0 || parent.height <= 0.0 {
        return;
    }

    let mut sizes = roots
        .iter()
        .map(|node| {
            let intrinsic = measure_node(node, parent.width, parent.height, world_id);
            (
                clamp_length(
                    node.layout.width.resolve(parent.width, intrinsic.0),
                    node.layout.max_width,
                    parent.width,
                ),
                clamp_length(
                    node.layout.height.resolve(parent.height, intrinsic.1),
                    node.layout.max_height,
                    parent.height,
                ),
            )
        })
        .collect::<Vec<_>>();
    let gap = SHARED_HEADER_CELL_GAP;
    let requested_width =
        sizes.iter().map(|size| size.0).sum::<f32>() + gap * roots.len().saturating_sub(1) as f32;
    if requested_width > parent.width && !sizes.is_empty() {
        let available = (parent.width - gap * roots.len().saturating_sub(1) as f32).max(0.0);
        let requested = sizes.iter().map(|size| size.0).sum::<f32>().max(1.0);
        for size in &mut sizes {
            size.0 = size.0 * available / requested;
        }
    }
    let total_width =
        sizes.iter().map(|size| size.0).sum::<f32>() + gap * roots.len().saturating_sub(1) as f32;
    let mut cursor = if centered {
        (parent.width - total_width).max(0.0) * 0.5
    } else {
        0.0
    };

    for (node, (width, height)) in roots.iter().zip(sizes) {
        let rect = UiRect {
            x: parent.x + cursor + node.layout.offset[0],
            y: parent.y + (parent.height - height).max(0.0) * 0.5 + node.layout.offset[1],
            width,
            height,
        };
        layout_node(node, rect, world_id, pressed, nodes, hit_regions);
        cursor += width + gap;
    }
}

fn layout_node(
    node: &UiNode,
    rect: UiRect,
    world_id: &str,
    pressed: &HashSet<&str>,
    nodes: &mut Vec<UiRenderNode>,
    hit_regions: &mut Vec<UiHitRegion>,
) {
    if !node_is_visible(node, world_id) {
        return;
    }
    let value = match node.kind {
        UiNodeKind::Toggle => f32::from(node.checked),
        UiNodeKind::Slider => ((node.value - node.minimum)
            / (node.maximum - node.minimum).max(f32::EPSILON))
        .clamp(0.0, 1.0),
        _ => node.value,
    };
    nodes.push(UiRenderNode {
        id: node.id.clone(),
        kind: node.kind,
        rect,
        text: node.text.clone(),
        icon: node.icon.clone(),
        background: node.style.background.as_deref().and_then(parse_color),
        foreground: parse_color(&node.style.foreground).unwrap_or([1.0; 4]),
        border_color: node.style.border_color.as_deref().and_then(parse_color),
        border_width: node.style.border_width.max(0.0),
        corner_radius: node.style.corner_radius.max(0.0),
        font_size: node.style.font_size.max(7.0),
        text_align: node.style.text_align,
        accent: node
            .style
            .accent
            .as_deref()
            .and_then(parse_color)
            .unwrap_or([0.0, 0.58, 1.0, 1.0]),
        image: None,
        image_invert: false,
        value,
        value_x: node.value_x,
        value_y: node.value_y,
        checked: node.checked,
        pressed: pressed.contains(node.id.as_str()),
        disabled: node.disabled,
    });
    let inherently_interactive = matches!(
        node.kind,
        UiNodeKind::Button | UiNodeKind::Toggle | UiNodeKind::Slider | UiNodeKind::Joystick
    );
    if inherently_interactive || node.blocks_input || !node.action.is_empty() {
        hit_regions.push(UiHitRegion {
            id: node.id.clone(),
            action: if node.action.is_empty() && inherently_interactive {
                node.id.clone()
            } else {
                node.action.clone()
            },
            kind: node.kind,
            rect,
            disabled: node.disabled,
        });
    }
    if node.children.is_empty() {
        return;
    }

    let content = rect.inset(node.layout.padding);
    let visible_children = node
        .children
        .iter()
        .filter(|child| node_is_visible(child, world_id))
        .collect::<Vec<_>>();
    if visible_children.is_empty() {
        return;
    }
    let gap = node.layout.gap.max(0.0);
    let main_available = match node.layout.direction {
        UiDirection::Row => content.width,
        UiDirection::Column => content.height,
    };
    let cross_available = match node.layout.direction {
        UiDirection::Row => content.height,
        UiDirection::Column => content.width,
    };
    let intrinsic = visible_children
        .iter()
        .map(|child| measure_node(child, content.width, content.height, world_id))
        .collect::<Vec<_>>();
    let fill_count = visible_children
        .iter()
        .filter(|child| match node.layout.direction {
            UiDirection::Row => child.layout.width.is_fill(),
            UiDirection::Column => child.layout.height.is_fill(),
        })
        .count();
    let fixed_main = visible_children
        .iter()
        .zip(&intrinsic)
        .filter(|(child, _)| match node.layout.direction {
            UiDirection::Row => !child.layout.width.is_fill(),
            UiDirection::Column => !child.layout.height.is_fill(),
        })
        .map(|(_, size)| match node.layout.direction {
            UiDirection::Row => size.0,
            UiDirection::Column => size.1,
        })
        .sum::<f32>();
    let base_gap = gap * visible_children.len().saturating_sub(1) as f32;
    let fill_main = if fill_count > 0 {
        ((main_available - fixed_main - base_gap).max(0.0)) / fill_count as f32
    } else {
        0.0
    };
    let used_main = if fill_count > 0 {
        main_available
    } else {
        fixed_main
            + intrinsic
                .iter()
                .enumerate()
                .filter_map(|(index, size)| {
                    let child = visible_children[index];
                    let is_fill = match node.layout.direction {
                        UiDirection::Row => child.layout.width.is_fill(),
                        UiDirection::Column => child.layout.height.is_fill(),
                    };
                    is_fill.then_some(match node.layout.direction {
                        UiDirection::Row => size.0,
                        UiDirection::Column => size.1,
                    })
                })
                .sum::<f32>()
            + base_gap
    };
    let mut actual_gap = gap;
    let mut cursor = match node.layout.justify {
        UiJustification::Center => (main_available - used_main).max(0.0) * 0.5,
        UiJustification::End => (main_available - used_main).max(0.0),
        UiJustification::SpaceBetween if visible_children.len() > 1 && fill_count == 0 => {
            actual_gap = ((main_available - (used_main - base_gap)).max(0.0))
                / visible_children.len().saturating_sub(1) as f32;
            0.0
        }
        _ => 0.0,
    };

    for (child, intrinsic) in visible_children.into_iter().zip(intrinsic) {
        let main_is_fill = match node.layout.direction {
            UiDirection::Row => child.layout.width.is_fill(),
            UiDirection::Column => child.layout.height.is_fill(),
        };
        let main = if main_is_fill {
            fill_main
        } else {
            match node.layout.direction {
                UiDirection::Row => intrinsic.0,
                UiDirection::Column => intrinsic.1,
            }
        };
        let child_cross_fill = match node.layout.direction {
            UiDirection::Row => child.layout.height.is_fill(),
            UiDirection::Column => child.layout.width.is_fill(),
        } || node.layout.align == UiAlignment::Stretch;
        let cross = if child_cross_fill {
            cross_available
        } else {
            match node.layout.direction {
                UiDirection::Row => intrinsic.1.min(cross_available),
                UiDirection::Column => intrinsic.0.min(cross_available),
            }
        };
        let cross_offset = match node.layout.align {
            UiAlignment::Center => (cross_available - cross) * 0.5,
            UiAlignment::End => cross_available - cross,
            _ => 0.0,
        };
        let child_rect = match node.layout.direction {
            UiDirection::Row => UiRect {
                x: content.x + cursor + child.layout.offset[0],
                y: content.y + cross_offset + child.layout.offset[1],
                width: main,
                height: cross,
            },
            UiDirection::Column => UiRect {
                x: content.x + cross_offset + child.layout.offset[0],
                y: content.y + cursor + child.layout.offset[1],
                width: cross,
                height: main,
            },
        };
        layout_node(child, child_rect, world_id, pressed, nodes, hit_regions);
        cursor += main + actual_gap;
    }
}

pub(crate) fn parse_color(value: &str) -> Option<[f32; 4]> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 && value.len() != 8 {
        return None;
    }
    let color = u32::from_str_radix(value, 16).ok()?;
    let (rgb, alpha) = if value.len() == 8 {
        (color >> 8, (color & 0xff) as f32 / 255.0)
    } else {
        (color, 1.0)
    };
    Some([
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
        alpha,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(source: &str, width: f32, height: f32) -> UiRuntime {
        let mut runtime = UiRuntime::default();
        runtime.set_viewport(UiViewport {
            width,
            height,
            scale: 1.0,
            safe_area: UiInsets {
                top: 47.0,
                right: 0.0,
                bottom: 34.0,
                left: 0.0,
            },
        });
        runtime.set_document_json(source).unwrap();
        runtime
    }

    #[test]
    fn anchors_to_safe_area_and_respects_max_width() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"header","kind":"panel","layout":{"anchor":"top","width":"fill","height":64,"maxWidth":720}}]}"##,
            1280.0,
            800.0,
        );
        let header = &runtime.frame().nodes[0];
        assert_eq!(
            header.rect,
            UiRect {
                x: 280.0,
                y: 47.0,
                width: 720.0,
                height: 64.0
            }
        );
    }

    #[test]
    fn shared_header_is_engine_owned_and_emits_no_event() {
        let mut runtime = runtime(r##"{"nodes":[]}"##, 390.0, 844.0);
        let frame = runtime.frame().clone();
        let shared_ids = frame
            .nodes
            .iter()
            .filter(|node| node.id.starts_with("__shared_header_"))
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(shared_ids.len(), 6);
        assert!(runtime.pointer(1, UiPointerPhase::Down, 32.0, 80.0));
        assert!(runtime.pointer(1, UiPointerPhase::Up, 32.0, 80.0));
        assert!(!runtime.poll_event());
    }

    #[test]
    fn shared_logo_opens_and_closes_placeholder_modal() {
        let mut runtime = runtime(r##"{"nodes":[]}"##, 390.0, 844.0);
        let logo = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.id == "__shared_header_logo_surface")
            .expect("shared logo should render")
            .rect;
        let logo_x = logo.x + logo.width * 0.5;
        let logo_y = logo.y + logo.height * 0.5;

        assert!(runtime.pointer(1, UiPointerPhase::Down, logo_x, logo_y));
        assert!(runtime.pointer(1, UiPointerPhase::Up, logo_x, logo_y));
        runtime.advance(1.0);
        let frame = runtime.frame().clone();
        for label in ["About", "Settings", "People", "Report", "Help"] {
            assert!(frame.nodes.iter().any(|node| node.text == label));
        }
        assert!(frame.nodes.iter().any(|node| node.id == "__shared_modal_scrim"));
        assert!(frame.nodes.iter().any(|node| node.id == "__shared_modal_body"));
        assert!(!runtime.poll_event(), "shared modal is engine-owned");

        // The scrim is above the header, so tapping the logo while open is
        // the same dismiss gesture as tapping anywhere outside the panel.
        assert!(runtime.pointer(2, UiPointerPhase::Down, logo_x, logo_y));
        assert!(runtime.pointer(2, UiPointerPhase::Up, logo_x, logo_y));
        runtime.advance(1.0);
        assert!(runtime
            .frame()
            .nodes
            .iter()
            .all(|node| !node.id.starts_with("__shared_modal_")));
    }

    #[test]
    fn shared_modal_tabs_are_selectable_without_script_events() {
        let mut runtime = runtime(r##"{"nodes":[]}"##, 1024.0, 768.0);
        let logo = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.id == "__shared_header_logo_surface")
            .expect("shared logo should render")
            .rect;
        let logo_x = logo.x + logo.width * 0.5;
        let logo_y = logo.y + logo.height * 0.5;
        assert!(runtime.pointer(1, UiPointerPhase::Down, logo_x, logo_y));
        assert!(runtime.pointer(1, UiPointerPhase::Up, logo_x, logo_y));
        runtime.advance(1.0);

        let people = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.text == "People")
            .expect("People placeholder tab should render")
            .rect;
        let people_x = people.x + people.width * 0.5;
        let people_y = people.y + people.height * 0.5;
        assert!(runtime.pointer(3, UiPointerPhase::Down, people_x, people_y));
        assert!(runtime.pointer(3, UiPointerPhase::Up, people_x, people_y));
        assert!(!runtime.poll_event(), "placeholder tabs are engine-owned");

        let people = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.text == "People")
            .unwrap();
        assert_eq!(people.background, Some([0.24, 0.30, 0.34, 0.98]));
    }

    #[test]
    fn game_header_region_follows_shared_header_and_bottom_region_is_compact() {
        let mut runtime = runtime(
            r##"{
                "nodes":[
                    {"id":"build","kind":"button","icon":"build","action":"build.menu","layout":{"region":"header","width":56,"height":56}},
                    {"id":"context","kind":"panel","layout":{"region":"bottomCenter","width":"auto","height":56,"padding":4,"direction":"row","gap":6},"children":[
                        {"id":"place","kind":"button","icon":"plus","action":"build.place","layout":{"width":48,"height":48}},
                        {"id":"rotate","kind":"button","icon":"rotate","action":"build.rotate","layout":{"width":48,"height":48}},
                        {"id":"remove","kind":"button","icon":"trash","action":"build.remove","layout":{"width":48,"height":48}}
                    ]}
                ]
            }"##,
            390.0,
            844.0,
        );
        let frame = runtime.frame().clone();
        let shared_controls = frame
            .nodes
            .iter()
            .find(|node| node.id == "__shared_header_controls_surface")
            .expect("shared controls should render");
        let build = frame
            .nodes
            .iter()
            .find(|node| node.id == "build")
            .expect("game header control should render");
        let context = frame
            .nodes
            .iter()
            .find(|node| node.id == "context")
            .expect("bottom context should render");
        assert!(build.rect.x >= shared_controls.rect.x + shared_controls.rect.width);
        assert_eq!(build.icon.as_deref(), Some("build"));
        assert_eq!(context.rect.width, 164.0);
        assert!(context.rect.x > 100.0 && context.rect.x + context.rect.width < 290.0);
        assert!(context.rect.y + context.rect.height <= 844.0 - 34.0);
    }

    #[test]
    fn visible_in_scopes_rendering_and_hit_testing_to_worlds() {
        let mut runtime = runtime(
            r##"{
                "nodes":[
                    {"id":"game-only","kind":"button","text":"BUILD","action":"build.place","visibleIn":["real-game"],"layout":{"width":120,"height":48,"offset":[0,140]}},
                    {"id":"game-panel","kind":"panel","visibleIn":["real-game"],"layout":{"width":160,"height":64,"offset":[140,140]},"children":[
                        {"id":"game-child","kind":"text","text":"GAME"}
                    ]}
                ]
            }"##,
            390.0,
            844.0,
        );

        assert!(
            runtime
                .frame()
                .nodes
                .iter()
                .all(|node| !matches!(node.id.as_str(), "game-only" | "game-panel" | "game-child"))
        );
        assert!(!runtime.pointer(1, UiPointerPhase::Down, 40.0, 210.0));

        runtime.set_world_id("real-game");
        let frame = runtime.frame().clone();
        assert!(frame.nodes.iter().any(|node| node.id == "game-only"));
        assert!(frame.nodes.iter().any(|node| node.id == "game-child"));
        assert!(runtime.pointer(1, UiPointerPhase::Down, 40.0, 210.0));
        assert!(runtime.pointer(1, UiPointerPhase::Up, 40.0, 210.0));
        assert!(runtime.poll_event());
        let event: serde_json::Value = serde_json::from_slice(runtime.event_buffer()).unwrap();
        assert_eq!(event["action"], "build.place");
    }

    #[test]
    fn gameplay_controls_are_persistent_across_worlds() {
        let mut runtime = runtime(
            r##"{
                "nodes":[
                    {"id":"player-joystick","kind":"joystick","action":"player.move","visible":false,"visibleIn":["real-game"],"layout":{"anchor":"bottomLeft","width":120,"height":120,"offset":[20,-24]},"style":{"background":"#091A22C9","foreground":"#EDF0E5FF"}},
                    {"id":"player-jump","kind":"button","text":"JUMP","action":"player.jump","visible":false,"visibleIn":["real-game"],"layout":{"anchor":"bottomRight","width":86,"height":44,"offset":[-22,-84]},"style":{"background":"#102D3AE8","foreground":"#F7F8EEFF"}},
                    {"id":"player-run","kind":"button","text":"RUN","action":"player.run","visible":false,"visibleIn":["real-game"],"layout":{"anchor":"bottomRight","width":86,"height":44,"offset":[-22,-30]},"style":{"background":"#102D3AE8","foreground":"#F7F8EEFF"}}
                ]
            }"##,
            1024.0,
            768.0,
        );
        let frame = runtime.frame().clone();
        for id in ["player-joystick", "player-jump", "player-run"] {
            assert!(
                frame.nodes.iter().any(|node| node.id == id),
                "persistent control {id} should render in the lobby"
            );
        }
        let joystick = frame
            .nodes
            .iter()
            .find(|node| node.id == "player-joystick")
            .expect("joystick should render");
        assert!(joystick.rect.x >= 20.0);
        assert!(joystick.rect.y + joystick.rect.height <= 768.0 - 34.0);

        let jump = frame
            .nodes
            .iter()
            .find(|node| node.id == "player-jump")
            .expect("jump should render");
        let jump_x = jump.rect.x + jump.rect.width * 0.5;
        let jump_y = jump.rect.y + jump.rect.height * 0.5;
        assert!(runtime.pointer(7, UiPointerPhase::Down, jump_x, jump_y));
        assert!(runtime.pointer(7, UiPointerPhase::Up, jump_x, jump_y));
        assert!(runtime.poll_event());
        assert!(std::str::from_utf8(runtime.event_buffer())
            .expect("event should be UTF-8")
            .contains("player.jump"));
    }

    #[test]
    fn portrait_ipad_uses_compact_shared_header_geometry() {
        let mut runtime = runtime(r##"{"nodes":[]}"##, 768.0, 1024.0);
        let frame = runtime.frame().clone();
        let logo = frame
            .nodes
            .iter()
            .find(|node| node.id == "__shared_header_logo_surface")
            .expect("portrait iPad should render the shared header");
        let controls = frame
            .nodes
            .iter()
            .find(|node| node.id == "__shared_header_controls_surface")
            .expect("portrait iPad should render shared controls");
        assert_eq!(logo.rect.width, 44.0);
        assert!(controls.rect.x + controls.rect.width < 768.0);
    }

    #[test]
    fn menu_and_modal_nodes_are_valid_container_primitives() {
        let mut runtime = runtime(
            r##"{"nodes":[
                {"id":"scrim","kind":"modal","blocksInput":true,"layout":{"width":"fill","height":"fill","ignoreSafeArea":true}},
                {"id":"menu","kind":"menu","layout":{"width":220,"height":180},"children":[
                    {"id":"cube","kind":"button","icon":"cube","text":"CUBE","action":"shape.cube","layout":{"width":96,"height":56}}
                ]}
            ]}"##,
            390.0,
            844.0,
        );
        let frame = runtime.frame().clone();
        assert_eq!(
            frame
                .nodes
                .iter()
                .find(|node| node.id == "scrim")
                .expect("scrim should render")
                .kind,
            UiNodeKind::Modal
        );
        assert_eq!(
            frame
                .nodes
                .iter()
                .find(|node| node.id == "menu")
                .expect("menu should render")
                .kind,
            UiNodeKind::Menu
        );
        assert_eq!(
            frame
                .nodes
                .iter()
                .find(|node| node.id == "cube")
                .expect("menu item should render")
                .icon
                .as_deref(),
            Some("cube")
        );
    }

    #[test]
    fn bottom_dock_stays_above_home_indicator() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"dock","kind":"panel","layout":{"anchor":"bottom","width":320,"height":56,"offset":[0,-16]}}]}"##,
            390.0,
            844.0,
        );
        let dock = &runtime.frame().nodes[0];
        assert_eq!(dock.rect.y, 738.0);
        assert!(dock.rect.y + dock.rect.height <= 844.0 - 34.0);
    }

    #[test]
    fn button_requires_release_inside_and_emits_host_event() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"build","kind":"button","text":"BUILD","action":"build.use","layout":{"width":120,"height":48,"offset":[0,140]}}]}"##,
            390.0,
            844.0,
        );
        assert!(runtime.pointer(7, UiPointerPhase::Down, 30.0, 200.0));
        assert!(runtime.pointer(7, UiPointerPhase::Up, 30.0, 200.0));
        assert!(runtime.poll_event());
        let event: serde_json::Value = serde_json::from_slice(runtime.event_buffer()).unwrap();
        assert_eq!(event["action"], "build.use");
        assert_eq!(event["phase"], "activate");
    }

    #[test]
    fn slider_updates_value_during_drag() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"music","kind":"slider","action":"settings.music","value":0.5,"layout":{"width":200,"height":44,"offset":[0,140]}}]}"##,
            390.0,
            844.0,
        );
        assert!(runtime.pointer(2, UiPointerPhase::Down, 100.0, 200.0));
        assert!(runtime.pointer(2, UiPointerPhase::Move, 180.0, 200.0));
        let slider = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.id == "music")
            .unwrap();
        assert!((slider.value - 0.9).abs() < 0.001);
    }

    #[test]
    fn rejects_duplicate_ids() {
        let mut runtime = UiRuntime::default();
        let error = runtime
            .set_document_json(r##"{"nodes":[{"id":"same"},{"id":"same"}]}"##)
            .unwrap_err();
        assert!(error.contains("Duplicate"));
    }

    #[test]
    fn modal_scrim_can_cover_unsafe_area_and_block_world_input() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"scrim","kind":"panel","blocksInput":true,"layout":{"width":"fill","height":"fill","ignoreSafeArea":true}}]}"##,
            390.0,
            844.0,
        );
        let scrim = &runtime.frame().nodes[0];
        assert_eq!(
            scrim.rect,
            UiRect {
                x: 0.0,
                y: 0.0,
                width: 390.0,
                height: 844.0
            }
        );
        assert!(runtime.pointer(11, UiPointerPhase::Down, 5.0, 5.0));
        assert!(runtime.pointer(11, UiPointerPhase::Up, 5.0, 5.0));
        assert!(!runtime.poll_event());
    }

    #[test]
    fn modal_overlay_takes_priority_over_shared_and_game_controls() {
        let mut runtime = runtime(
            r##"{"nodes":[
                {"id":"underlay","kind":"button","text":"UNDER","action":"underlay","layout":{"width":120,"height":48,"offset":[0,140]}},
                {"id":"scrim","kind":"modal","action":"close","blocksInput":true,"layout":{"width":"fill","height":"fill","ignoreSafeArea":true}}
            ]}"##,
            390.0,
            844.0,
        );
        assert!(runtime.pointer(9, UiPointerPhase::Down, 30.0, 200.0));
        assert!(runtime.pointer(9, UiPointerPhase::Up, 30.0, 200.0));
        assert!(runtime.poll_event());
        let event = std::str::from_utf8(runtime.event_buffer()).expect("event should be UTF-8");
        assert!(event.contains("\"action\":\"close\""));
    }

    #[test]
    fn oversized_popover_is_clamped_inside_the_safe_viewport() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"popover","kind":"menu","layout":{"width":336,"height":136,"offset":[380,88]}}]}"##,
            390.0,
            844.0,
        );
        let popover = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.id == "popover")
            .unwrap();
        assert!(popover.rect.x >= 0.0);
        assert!(popover.rect.x + popover.rect.width <= 390.0);
        assert!(popover.rect.y >= 47.0);
        assert!(popover.rect.y + popover.rect.height <= 810.0);
    }

    #[test]
    fn joystick_clamps_vector_and_resets_on_release() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"move","kind":"joystick","action":"player.move","layout":{"width":120,"height":120,"offset":[0,140]}}]}"##,
            390.0,
            844.0,
        );
        assert!(runtime.pointer(4, UiPointerPhase::Down, 60.0, 247.0));
        assert!(runtime.pointer(4, UiPointerPhase::Move, 180.0, 247.0));
        let stick = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.id == "move")
            .unwrap();
        assert_eq!(stick.value_x, 1.0);
        assert_eq!(stick.value_y, 0.0);
        assert!(runtime.pointer(4, UiPointerPhase::Up, 180.0, 247.0));
        let stick = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.id == "move")
            .unwrap();
        assert_eq!((stick.value_x, stick.value_y), (0.0, 0.0));
        assert!(runtime.host_events.iter().any(|event| {
            event.phase == "release" && event.x == Some(0.0) && event.y == Some(0.0)
        }));
    }
}
