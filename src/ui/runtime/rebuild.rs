use std::collections::HashSet;

use super::*;

impl UiRuntime {
    pub(super) fn rebuild_if_needed(&mut self) {
        if !self.dirty {
            return;
        }
        if self.suppressed {
            self.frame = UiFrame {
                viewport: self.viewport,
                nodes: Vec::new(),
            };
            self.hit_regions.clear();
            self.dirty = false;
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
}
