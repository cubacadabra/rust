use crate::character::{AnimationOutput, CharacterPresentationState};
use crate::character::{
    AppearanceInput, CharacterAppearance, CharacterColors, OutfitId, resolve_appearance,
};
use crate::engine::Engine;
use crate::game_package::{AvatarDefinition, GamePackageDefinition, WorldDefinition};
use crate::types::{CharacterEntityKind, CharacterMotionSample};
#[cfg(target_os = "ios")]
use crate::ui::UiFrame;
use cubacadabra_morphs::MorphAssetId;
use std::collections::HashSet;

#[cfg(target_os = "ios")]
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    AvatarStyle, RenderBillboard, RenderBlock, RenderCloud, RenderEntity, RenderInteraction,
    RenderLadder, RenderPad, RenderPalette, RenderSign, RenderWorld, Renderer,
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
                        .map(|(_, world)| resolve_world(&world, &package.effects))
                        .collect()
                })
                .unwrap_or_default();
            self.package_generation = engine.package_generation;
            self.active_world = usize::MAX;
        }

        if self.active_world != engine.active_world
            && let Some(world) = self.worlds.get(engine.active_world).cloned()
        {
            self.scene.presentation.clear();
            self.scene.lods.clear();
            self.active_world = engine.active_world;
            self.scene.world = world;
            self.rebuild_static_vertices();
        }

        self.scene.player = RenderEntity::default();
        self.scene.agents.clear();
        self.scene.remote_players.clear();
        self.scene.remote_names.clear();
        self.scene.morph_assets.clear();
        self.scene.player_style =
            style_from_appearance(engine.player_appearance(), self.scene.player_style);
        let reduced_effects = engine.reduced_effects();
        if self.scene.reduced_effects != reduced_effects {
            // A quality preference is presentation state, but changing it
            // must affect the next sync even when the simulation tick is
            // unchanged. Rebuilding from the current sample cannot replay
            // an event because no sample history is inferred on first use.
            self.scene.presentation.clear();
            self.scene.lods.clear();
            self.scene.reduced_effects = reduced_effects;
        }
        let samples: Vec<_> = engine.character_motion_samples().collect();
        let mut active_keys = HashSet::with_capacity(samples.len());
        let mut remote_index = 0;
        for sample in samples {
            active_keys.insert(sample.key);
            let style = match sample.key.kind {
                CharacterEntityKind::LocalNpc => self
                    .scene
                    .npc_styles
                    .get(sample.key.slot % self.scene.npc_styles.len().max(1))
                    .copied()
                    .unwrap_or(self.scene.player_style),
                CharacterEntityKind::LocalPlayer => self.scene.player_style,
                CharacterEntityKind::RemotePlayer => engine
                    .remote_appearance(sample.key)
                    .map(|appearance| style_from_appearance(appearance, self.scene.player_style))
                    .unwrap_or(self.scene.player_style),
            };
            let body = style.body;
            let morph_assets = match sample.key.kind {
                CharacterEntityKind::LocalPlayer => {
                    morph_assets_for_appearance(engine.player_appearance())
                }
                CharacterEntityKind::RemotePlayer => engine
                    .remote_appearance(sample.key)
                    .map(morph_assets_for_appearance)
                    .unwrap_or_default(),
                CharacterEntityKind::LocalNpc => Vec::new(),
            };
            if !morph_assets.is_empty() {
                self.scene.morph_assets.insert(sample.key, morph_assets);
            }
            let reduced_effects = self.scene.reduced_effects;
            let presentation = self
                .scene
                .presentation
                .entry(sample.key)
                .or_insert_with(|| CharacterPresentationState::new(sample.key, body));
            presentation.set_expression(style.face);
            let animation = presentation.evaluate(sample, body, reduced_effects);
            match sample.key.kind {
                CharacterEntityKind::LocalPlayer => {
                    self.scene.player = render_entity(sample, style, animation)
                }
                CharacterEntityKind::LocalNpc => self
                    .scene
                    .agents
                    .push(render_entity(sample, style, animation)),
                CharacterEntityKind::RemotePlayer => self
                    .scene
                    .remote_players
                    .push(render_entity(sample, style, animation)),
            }
            if sample.key.kind == CharacterEntityKind::RemotePlayer {
                let fallback = format!("PLAYER {}", remote_index + 1);
                let name = engine
                    .remote_players
                    .get(remote_index)
                    .map(|player| {
                        let source = if player.display_name.trim().is_empty() {
                            &player.stable_id
                        } else {
                            &player.display_name
                        };
                        display_name(source, &fallback)
                    })
                    .unwrap_or(fallback);
                self.scene.remote_names.push(name);
                remote_index += 1;
            }
        }
        self.scene
            .presentation
            .retain(|key, _| active_keys.contains(key));
        self.scene.lods.retain(|key, _| active_keys.contains(key));
        self.scene
            .morph_assets
            .retain(|key, _| active_keys.contains(key));
        self.scene.pad_seconds.clear();
        self.scene
            .pad_seconds
            .extend((0..engine.launch_pad_count()).map(|index| engine.launch_pad_seconds(index)));
        self.scene.camera = engine.camera();
        self.scene.elapsed = engine.elapsed();
        self.scene.interaction_states.clear();
        self.scene.interaction_states.extend(
            (0..engine.interaction_count()).map(|index| engine.interaction_render_state(index)),
        );
        self.scene.effect_states.clear();
        self.scene.effect_states.extend(
            engine
                .effects
                .states
                .iter()
                .filter(|((world, _), _)| *world == engine.active_world)
                .map(|((_, target), state)| (target.clone(), state.clone())),
        );
        self.scene.effect_instances.clear();
        self.scene.effect_instances.extend(
            engine
                .effects
                .instances
                .iter()
                .filter(|instance| instance.world == engine.active_world)
                .cloned(),
        );
        self.scene.username.clone_from(&engine.username);
        self.scene.build_blocks.clear();
        self.scene
            .build_blocks
            .extend_from_slice(engine.build_blocks());
        if self.avatar_preview_mode {
            self.apply_avatar_preview_scene();
        }
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

    fn apply_avatar_preview_scene(&mut self) {
        self.scene.world = RenderWorld {
            ground_size: 12.0,
            grid_size: 0.0,
            grid_divisions: 0,
            show_grid: false,
            show_spawn_pad: false,
            palette: super::RenderPalette {
                sky: [0.035, 0.04, 0.07, 1.0],
                ground: [0.07, 0.08, 0.12, 1.0],
                ground_edge: [0.10, 0.12, 0.18, 1.0],
                grid: [0.10, 0.12, 0.18, 1.0],
                ink: [0.85, 0.88, 0.98, 1.0],
                paper: [0.96, 0.97, 1.0, 1.0],
            },
            ..RenderWorld::default()
        };
        self.scene.agents.clear();
        self.scene.remote_players.clear();
        self.scene.remote_names.clear();
        self.scene.pad_seconds.clear();
        self.scene.interaction_states.clear();
        self.scene.effect_states.clear();
        self.scene.effect_instances.clear();
        self.scene.build_blocks.clear();
        self.scene.player.position = [0.0, 0.0, 0.0];
        self.scene.player.yaw = 0.0;
        self.scene.player.walk_cycle = 0.0;
        self.scene.player.moving = false;
        self.scene.player.sprinting = false;
        self.scene.player.support = crate::types::CharacterSupport::Grounded { height: 0.0 };
        self.scene.camera = [0.0, -0.06, 5.8];
        self.rebuild_static_vertices();
    }
}

fn morph_assets_for_appearance(appearance: &CharacterAppearance) -> Vec<MorphAssetId> {
    appearance
        .equipment
        .iter()
        .filter_map(|item| MorphAssetId::parse(&item.asset_id).ok())
        .collect()
}

fn display_name(value: &str, fallback: &str) -> String {
    let name = value
        .rsplit(':')
        .next()
        .unwrap_or(value)
        .chars()
        .filter(|character| character.is_ascii_graphic() || *character == ' ')
        .take(24)
        .collect::<String>();
    if name.trim().is_empty() {
        fallback.to_owned()
    } else {
        name
    }
}

fn render_entity(
    sample: CharacterMotionSample,
    style: AvatarStyle,
    animation: AnimationOutput,
) -> RenderEntity {
    RenderEntity {
        key: sample.key,
        position: sample.position,
        yaw: sample.facing_yaw,
        walk_cycle: sample.stride_phase,
        moving: sample.moving,
        sprinting: sample.sprinting,
        legacy_assembled: false,
        body: style.body,
        outfit: style.outfit,
        pose: animation.pose,
        face: animation.face,
        secondary: animation.secondary,
        support: sample.support,
        camera_fade: 0.0,
        style,
    }
}

fn resolve_world(
    definition: &WorldDefinition,
    effects: &crate::effects::EffectLibraryDefinition,
) -> RenderWorld {
    let defaults = RenderPalette::default();
    let palette = RenderPalette {
        sky: resolve_color(&definition.palette, "sky", defaults.sky),
        ground: resolve_color(&definition.palette, "ground", defaults.ground),
        ground_edge: resolve_color(&definition.palette, "groundEdge", defaults.ground_edge),
        grid: resolve_color(&definition.palette, "grid", defaults.grid),
        ink: resolve_color(&definition.palette, "ink", defaults.ink),
        paper: resolve_color(&definition.palette, "paper", defaults.paper),
    };
    let resolve_material = |id: &str| {
        definition.materials.get(id).and_then(|material| {
            (!material.image.is_empty()).then(|| super::RenderMaterial {
                image: material.image.clone(),
                tile_u: material.tile_u.max(0.05),
                tile_v: material.tile_v.max(0.05),
            })
        })
    };
    RenderWorld {
        ground_material: definition
            .ground_material
            .as_deref()
            .and_then(resolve_material),
        blocks: definition
            .blocks
            .iter()
            .map(|block| RenderBlock {
                position: block.position(),
                size: block.size(),
                color: resolve_color(&definition.palette, &block.color, super::color(0xffffff)),
                material: block.material.as_deref().and_then(resolve_material),
                outline: block.outline,
            })
            .collect(),
        ladders: definition
            .ladders
            .iter()
            .map(|ladder| RenderLadder {
                position: ladder.position(),
                size: ladder.size(),
                axis: if ladder.climb_axis.eq_ignore_ascii_case("x") {
                    crate::world::LadderAxis::X
                } else {
                    crate::world::LadderAxis::Z
                },
                color: resolve_color(&definition.palette, &ladder.color, palette.paper),
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
        ground_y: definition.world.physics.ground_y,
        grid_size: definition.world.grid_size.max(1.0),
        grid_divisions: definition.world.grid_divisions,
        show_grid: definition.world.show_grid,
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
        billboards: definition
            .billboards
            .iter()
            .map(|billboard| RenderBillboard {
                image: billboard.image.clone(),
                position: billboard.position(),
                yaw: billboard.yaw,
                width: billboard.width.max(0.5),
                height: billboard.height.max(0.5),
                framed: billboard.framed,
            })
            .collect(),
        interactions: definition
            .interactions
            .iter()
            .map(|interaction| RenderInteraction {
                id: interaction.id.clone(),
                label: if interaction.label.is_empty() {
                    interaction.id.clone()
                } else {
                    interaction.label.clone()
                },
                position: interaction.position(),
                radius: interaction.radius.max(0.5),
                color: resolve_color(&definition.palette, &interaction.color, palette.paper),
                visual: interaction.visual.clone(),
            })
            .collect(),
        effect_templates: super::effects::resolve_templates(effects, &definition.palette, palette),
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
    let legacy = CharacterColors {
        skin: definition
            .skin
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(fallback.skin),
        primary: definition
            .shirt
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(fallback.shirt),
        secondary: definition
            .pants
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(fallback.pants),
        sole: definition
            .shoes
            .as_deref()
            .and_then(parse_hex_color)
            .unwrap_or(fallback.shoes),
    };
    let resolution = definition.character.as_ref().map(|character| {
        let _bounded = character.bounded();
        resolve_appearance(AppearanceInput {
            version: character.version,
            body: character.body.as_deref(),
            face: character.face.as_deref(),
            outfit: character.outfit.as_deref(),
            equipment: &character.equipment,
            colors: &character.colors,
            legacy_colors: legacy,
            revision: character.revision,
        })
    });
    let appearance = resolution
        .map(|resolution| resolution.appearance)
        .unwrap_or_else(|| {
            resolve_appearance(AppearanceInput {
                version: Some(1),
                body: Some(fallback.body.stable_id()),
                face: Some(fallback.face.stable_id()),
                outfit: Some(fallback.outfit.stable_id()),
                equipment: &std::collections::BTreeMap::new(),
                colors: &std::collections::BTreeMap::new(),
                legacy_colors: legacy,
                revision: 0,
            })
            .appearance
        });
    AvatarStyle {
        skin: appearance.colors.skin,
        shirt: appearance.colors.primary,
        pants: appearance.colors.secondary,
        shoes: appearance.colors.sole,
        body: appearance.body,
        outfit: if appearance.outfit.supported_by(appearance.body) {
            appearance.outfit
        } else {
            OutfitId::fallback()
        },
        face: appearance.face,
    }
}

fn style_from_appearance(appearance: &CharacterAppearance, fallback: AvatarStyle) -> AvatarStyle {
    let outfit = appearance.outfit.supported_by(appearance.body);
    AvatarStyle {
        skin: appearance.colors.skin,
        shirt: appearance.colors.primary,
        pants: appearance.colors.secondary,
        shoes: appearance.colors.sole,
        body: appearance.body,
        outfit: outfit
            .then_some(appearance.outfit)
            .unwrap_or(fallback.outfit),
        face: appearance.face,
    }
}

pub(super) fn resolve_color(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::definition::{EquipmentItem, EquipmentSlot};
    use crate::character::{BodyId, FacePreset};

    #[test]
    fn appearance_keeps_multiple_registered_equipment_assets() {
        let appearance = CharacterAppearance {
            version: 1,
            body: BodyId::Person,
            face: FacePreset::Happy,
            outfit: OutfitId::EverydayHoodie,
            equipment: vec![
                EquipmentItem {
                    slot: EquipmentSlot::Hat,
                    asset_id: "cuba:headwear/test-top-hat.v1".to_owned(),
                },
                EquipmentItem {
                    slot: EquipmentSlot::EarAccessory,
                    asset_id: "cuba:headwear/headphones.v1".to_owned(),
                },
            ],
            colors: CharacterColors::default(),
            revision: 1,
        };

        let assets = morph_assets_for_appearance(&appearance);
        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].as_str(), "cuba:headwear/test-top-hat.v1");
        assert_eq!(assets[1].as_str(), "cuba:headwear/headphones.v1");
    }
}
