use crate::engine::{Engine, SNAPSHOT_STRIDE};
use crate::game_package::{AvatarDefinition, GamePackageDefinition, WorldDefinition};
#[cfg(target_os = "ios")]
use crate::ui::UiFrame;

#[cfg(target_os = "ios")]
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    AvatarStyle, RenderBlock, RenderCloud, RenderEntity, RenderPad, RenderPalette, RenderSign,
    RenderWorld, Renderer,
};

#[cfg(target_os = "ios")]
static LAST_UI_FRAME_SIGNATURE: AtomicU64 = AtomicU64::new(u64::MAX);

#[cfg(target_os = "ios")]
fn log_ui_frame(frame: &UiFrame) {
    let signature = (frame.nodes.len() as u64)
        ^ u64::from(frame.viewport.width.to_bits()).rotate_left(17)
        ^ u64::from(frame.viewport.height.to_bits()).rotate_left(41);
    if LAST_UI_FRAME_SIGNATURE.swap(signature, Ordering::Relaxed) == signature {
        return;
    }
    let vertices = super::ui::build_ui_vertices(frame).len();
    log::trace!(
        "[RustRenderer] UI frame viewport={:.1}x{:.1} safe=({:.1},{:.1},{:.1},{:.1}) nodes={} vertices={}",
        frame.viewport.width,
        frame.viewport.height,
        frame.viewport.safe_area.top,
        frame.viewport.safe_area.right,
        frame.viewport.safe_area.bottom,
        frame.viewport.safe_area.left,
        frame.nodes.len(),
        vertices,
    );
    for node in &frame.nodes {
        log::trace!(
            "[RustRenderer] UI node id={} kind={:?} rect=({:.1},{:.1},{:.1},{:.1}) text={:?}",
            node.id,
            node.kind,
            node.rect.x,
            node.rect.y,
            node.rect.width,
            node.rect.height,
            node.text,
        );
    }
}

impl Renderer {
    pub fn sync_engine(&mut self, engine: &Engine) {
        if self.package_generation != engine.package_generation {
            self.worlds = engine
                .package
                .as_ref()
                .map(|package| {
                    let (player_style, npc_styles) = resolve_avatar_styles(package);
                    self.scene.player_style = player_style;
                    self.scene.npc_styles = npc_styles;
                    package
                        .world_entries()
                        .into_iter()
                        .map(|(_, world)| resolve_world(&world))
                        .collect()
                })
                .unwrap_or_default();
            self.package_generation = engine.package_generation;
            self.active_world = usize::MAX;
        }

        if self.active_world != engine.active_world
            && let Some(world) = self.worlds.get(engine.active_world).cloned()
        {
            self.active_world = engine.active_world;
            self.scene.world = world;
            self.rebuild_static_vertices();
        }

        let snapshot = engine.snapshot();
        self.scene.player = render_entity(snapshot.get(..SNAPSHOT_STRIDE).unwrap_or(&[]));
        self.scene.agents.clear();
        self.scene.remote_players.clear();
        let local_agent_count = engine.local_agent_count();
        self.scene.agents.extend(
            snapshot
                .as_chunks::<SNAPSHOT_STRIDE>()
                .0
                .iter()
                .skip(1)
                .take(local_agent_count)
                .map(|values| render_entity(values)),
        );
        self.scene.remote_players.extend(
            snapshot
                .as_chunks::<SNAPSHOT_STRIDE>()
                .0
                .iter()
                .skip(local_agent_count + 1)
                .take(engine.remote_player_count())
                .map(|values| render_entity(values)),
        );
        self.scene.pad_seconds.clear();
        self.scene
            .pad_seconds
            .extend((0..engine.launch_pad_count()).map(|index| engine.launch_pad_seconds(index)));
        self.scene.camera = engine.camera();
        self.scene.elapsed = engine.elapsed();
        self.scene.username.clone_from(&engine.username);
        self.scene.build_blocks.clear();
        self.scene
            .build_blocks
            .extend_from_slice(engine.build_blocks());
        // SwiftUI/Metal can ask the renderer to sync while the engine is
        // still rebuilding its UI frame. Do not turn that transient overlap
        // into a process-aborting RefCell panic; the next frame will retry
        // with the latest UI state.
        let Ok(mut ui) = engine.ui.try_borrow_mut() else {
            return;
        };
        self.ui_frame = ui.frame().clone();
        #[cfg(target_os = "ios")]
        log_ui_frame(&self.ui_frame);
    }
}

fn render_entity(values: &[f32]) -> RenderEntity {
    let value = |index: usize| values.get(index).copied().unwrap_or(0.0);
    RenderEntity {
        position: [value(0), value(1), value(2)],
        yaw: value(3),
        walk_cycle: value(4),
        assembled: value(7),
    }
}

fn resolve_world(definition: &WorldDefinition) -> RenderWorld {
    let defaults = RenderPalette::default();
    let palette = RenderPalette {
        sky: resolve_color(&definition.palette, "sky", defaults.sky),
        ground: resolve_color(&definition.palette, "ground", defaults.ground),
        ground_edge: resolve_color(&definition.palette, "groundEdge", defaults.ground_edge),
        grid: resolve_color(&definition.palette, "grid", defaults.grid),
        ink: resolve_color(&definition.palette, "ink", defaults.ink),
        paper: resolve_color(&definition.palette, "paper", defaults.paper),
    };
    RenderWorld {
        blocks: definition
            .blocks
            .iter()
            .map(|block| RenderBlock {
                position: block.position(),
                size: block.size(),
                color: resolve_color(&definition.palette, &block.color, super::color(0xffffff)),
                outline: block.outline,
            })
            .collect(),
        pads: definition
            .launch_pads
            .iter()
            .map(|pad| RenderPad {
                x: pad.x(),
                z: pad.z(),
                radius: pad.radius.max(0.2),
                code: pad.code.clone(),
                label: pad.label.clone(),
                color: resolve_color(&definition.palette, &pad.color, palette.paper),
                enabled: pad.enabled,
                availability_label: if pad.availability_label.is_empty() {
                    "COMING SOON".to_owned()
                } else {
                    pad.availability_label.clone()
                },
            })
            .collect(),
        clouds: definition
            .world
            .clouds
            .iter()
            .map(|cloud| RenderCloud {
                position: cloud.position(),
                scale: cloud.scale.max(0.1),
            })
            .collect(),
        ground_size: definition.world.ground_size.max(10.0),
        grid_size: definition.world.grid_size.max(1.0),
        grid_divisions: definition.world.grid_divisions,
        spawn: definition.world.spawn(),
        show_spawn_pad: definition.world.show_spawn_pad,
        palette,
        signs: definition
            .signs
            .iter()
            .map(|sign| RenderSign {
                text: sign.text.clone(),
                position: sign.position(),
                yaw: sign.yaw,
                max_width: sign.max_width.max(0.2),
                color: resolve_color(&definition.palette, &sign.color, palette.paper),
            })
            .collect(),
    }
}

fn resolve_avatar_styles(package: &GamePackageDefinition) -> (AvatarStyle, Vec<AvatarStyle>) {
    let player = package
        .avatars
        .player
        .as_ref()
        .map_or_else(super::default_player_style, |style| {
            resolve_avatar_style(style, super::default_player_style())
        });
    let npcs = if package.avatars.npcs.is_empty() {
        super::default_npc_styles()
    } else {
        let defaults = super::default_npc_styles();
        package
            .avatars
            .npcs
            .iter()
            .enumerate()
            .map(|(index, style)| resolve_avatar_style(style, defaults[index % defaults.len()]))
            .collect()
    };
    (player, npcs)
}

fn resolve_avatar_style(definition: &AvatarDefinition, fallback: AvatarStyle) -> AvatarStyle {
    AvatarStyle {
        skin: definition
            .skin
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(fallback.skin),
        shirt: definition
            .shirt
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(fallback.shirt),
        pants: definition
            .pants
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(fallback.pants),
        shoes: definition
            .shoes
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(fallback.shoes),
    }
}

fn resolve_color(
    palette: &std::collections::BTreeMap<String, String>,
    token: &str,
    fallback: [f32; 4],
) -> [f32; 4] {
    palette
        .get(token)
        .map(String::as_str)
        .or_else(|| token.starts_with('#').then_some(token))
        .and_then(parse_hex_color)
        .unwrap_or(fallback)
}

fn parse_hex_color(value: &str) -> Option<[f32; 4]> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 {
        return None;
    }
    u32::from_str_radix(value, 16).ok().map(super::color)
}
