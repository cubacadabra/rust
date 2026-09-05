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
            shared_authenticated: false,
            shared_modal_progress: 0.0,
            shared_modal_target: 0.0,
            shared_modal_tab: 0,
            joystick_gesture_rect: UiRect::default(),
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
        self.joystick_gesture_rect = UiRect {
            width: safe.width * 0.5,
            ..safe
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
            self.shared_authenticated,
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

    fn joystick_gesture_contains(&self, region: &UiHitRegion, x: f32, y: f32) -> bool {
        region.id == "player-joystick"
            && region.kind == UiNodeKind::Joystick
            && self.joystick_gesture_rect.contains(x, y)
    }

    fn update_pointer_control(&mut self, pointer_id: u64, x: f32, y: f32, phase: &str) {
        let Some(capture) = self.captures.get(&pointer_id).cloned() else {
            return;
        };
        match capture.region.kind {
            UiNodeKind::Slider => self.update_slider(&capture.region, x, phase),
            UiNodeKind::Joystick => self.update_joystick(
                &capture.region,
                x,
                y,
                phase,
                capture.joystick_origin,
            ),
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

    fn update_joystick(
        &mut self,
        region: &UiHitRegion,
        x: f32,
        y: f32,
        phase: &str,
        origin: Option<(f32, f32)>,
    ) {
        let radius = region.rect.width.min(region.rect.height).max(1.0) * 0.5;
        let (origin_x, origin_y) = origin.unwrap_or((
            region.rect.x + region.rect.width * 0.5,
            region.rect.y + region.rect.height * 0.5,
        ));
        let mut value_x = (x - origin_x) / radius;
        let mut value_y = (y - origin_y) / radius;
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
