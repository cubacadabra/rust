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
            #[cfg(feature = "studio-ui")]
            document_revision: 0,
            shared_authenticated: false,
            shared_modal_progress: 0.0,
            shared_modal_target: 0.0,
            shared_modal_tab: 0,
            joystick_gesture_rect: UiRect::default(),
            suppressed: false,
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

    #[cfg(feature = "studio-ui")]
    pub(crate) fn document_revision(&self) -> u64 {
        self.document_revision
    }

    #[cfg(feature = "studio-ui")]
    fn bump_document_revision(&mut self) {
        self.document_revision = self.document_revision.wrapping_add(1);
    }

    #[cfg(not(feature = "studio-ui"))]
    fn bump_document_revision(&mut self) {}

    #[cfg(feature = "studio-ui")]
    pub(crate) fn studio_nodes(&self) -> Vec<crate::StudioUiNode> {
        fn collect(nodes: &[UiNode], output: &mut Vec<crate::StudioUiNode>) {
            for node in nodes {
                output.push(crate::StudioUiNode {
                    id: node.id.clone(),
                    kind: format!("{:?}", node.kind),
                    text: node.text.clone(),
                });
                collect(&node.children, output);
            }
        }

        let mut nodes = Vec::new();
        collect(&self.document.nodes, &mut nodes);
        nodes
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

    pub(crate) fn set_suppressed(&mut self, suppressed: bool) {
        if self.suppressed == suppressed {
            return;
        }
        self.suppressed = suppressed;
        self.captures.clear();
        self.dirty = true;
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
        self.bump_document_revision();
        self.captures.clear();
        self.dirty = true;
        Ok(())
    }

    pub(crate) fn clear(&mut self) {
        self.document.nodes.clear();
        self.bump_document_revision();
        self.captures.clear();
        self.dirty = true;
    }

    pub(crate) fn set_text(&mut self, id: &str, text: &str) -> bool {
        let Some(node) = find_node_mut(&mut self.document.nodes, id) else {
            return false;
        };
        node.text = text.chars().take(256).collect();
        self.bump_document_revision();
        self.dirty = true;
        true
    }

    pub(crate) fn set_value(&mut self, id: &str, value: f32) -> bool {
        let Some(node) = find_node_mut(&mut self.document.nodes, id) else {
            return false;
        };
        node.value = value.clamp(node.minimum, node.maximum.max(node.minimum));
        self.bump_document_revision();
        self.dirty = true;
        true
    }

    pub(crate) fn set_checked(&mut self, id: &str, checked: bool) -> bool {
        let Some(node) = find_node_mut(&mut self.document.nodes, id) else {
            return false;
        };
        node.checked = checked;
        node.value = f32::from(checked);
        self.bump_document_revision();
        self.dirty = true;
        true
    }

    pub(crate) fn set_visible(&mut self, id: &str, visible: bool) -> bool {
        let Some(node) = find_node_mut(&mut self.document.nodes, id) else {
            return false;
        };
        node.visible = visible;
        self.bump_document_revision();
        self.dirty = true;
        true
    }

    pub(crate) fn frame(&mut self) -> &UiFrame {
        self.rebuild_if_needed();
        &self.frame
    }

    pub(crate) fn shared_modal_visible(&self) -> bool {
        self.shared_modal_progress > 0.0 || self.shared_modal_target > 0.0
    }

    pub(crate) fn set_authenticated(&mut self, authenticated: bool) {
        if self.shared_authenticated == authenticated {
            return;
        }
        self.shared_authenticated = authenticated;
        self.dirty = true;
    }

    pub(crate) fn is_interactive_at(&mut self, x: f32, y: f32) -> bool {
        self.rebuild_if_needed();
        let is_interactive = |region: &UiHitRegion| {
            !region.disabled
                && (matches!(
                    region.kind,
                    UiNodeKind::Button
                        | UiNodeKind::Toggle
                        | UiNodeKind::Slider
                        | UiNodeKind::Joystick
                ) || !region.action.is_empty())
        };
        self.hit_regions
            .iter()
            .rev()
            .any(|region| is_interactive(region) && region.rect.contains(x, y))
            || self.hit_regions.iter().rev().any(|region| {
                is_interactive(region) && self.joystick_gesture_contains(region, x, y)
            })
    }

    pub(crate) fn is_external_link_at(&mut self, x: f32, y: f32) -> bool {
        self.rebuild_if_needed();
        self.hit_regions.iter().rev().any(|region| {
            !region.disabled
                && region.rect.contains(x, y)
                && region.action == "shared.about.open"
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
                    .or_else(|| {
                        self.hit_regions.iter().rev().find(|region| {
                            !region.disabled && self.joystick_gesture_contains(region, x, y)
                        })
                    })
                    .cloned()
                else {
                    return false;
                };
                let joystick_origin = (region.kind == UiNodeKind::Joystick).then_some((x, y));
                self.captures.insert(
                    pointer_id,
                    UiCapture {
                        region,
                        joystick_origin,
                    },
                );
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

}

mod input {
    include!("runtime/input.rs");
}

mod rebuild {
    include!("runtime/rebuild.rs");
}
