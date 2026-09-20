use super::{
    find_node_mut, shared_modal_tab_index, UiEvent, UiHitRegion, UiNodeKind, UiRuntime,
};

impl UiRuntime {
    pub(super) fn joystick_gesture_contains(&self, region: &UiHitRegion, x: f32, y: f32) -> bool {
        region.id == "player-joystick"
            && region.kind == UiNodeKind::Joystick
            && self.joystick_gesture_rect.contains(x, y)
    }

    pub(super) fn update_pointer_control(&mut self, pointer_id: u64, x: f32, y: f32, phase: &str) {
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

    pub(super) fn reset_joystick(&mut self, region: &UiHitRegion, phase: &str) {
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

    pub(super) fn activate(&mut self, region: UiHitRegion, x: f32) {
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
