use glam::{Mat4, Quat, Vec3};

use crate::effects::{
    valid_effect_id, EffectLibraryDefinition, EFFECTS_VERSION, MAX_EFFECT_NODES,
    MAX_EFFECT_NODE_COPIES, MAX_EFFECT_TEMPLATES,
};
use crate::types::InteractionRenderState;

use super::{
    faded, RenderEffectNode, RenderEffectTemplate, RenderInteraction, RenderPalette, Vertex,
};

pub(super) fn resolve_templates(
    library: &EffectLibraryDefinition,
    palette: &std::collections::BTreeMap<String, String>,
    render_palette: RenderPalette,
) -> std::collections::BTreeMap<String, RenderEffectTemplate> {
    if library.version != EFFECTS_VERSION {
        return std::collections::BTreeMap::new();
    }
    library
        .templates
        .iter()
        .filter(|(id, _)| valid_effect_id(id))
        .take(MAX_EFFECT_TEMPLATES)
        .map(|(id, template)| {
            let nodes = template
                .nodes
                .iter()
                .filter(|node| {
                    matches!(node.shape.as_str(), "box" | "cylinder" | "ring" | "sphere")
                })
                .take(MAX_EFFECT_NODES)
                .map(|node| {
                    let position = node
                        .position()
                        .map(|value| finite_clamp(value, -10_000.0, 10_000.0, 0.0));
                    let size = node
                        .size()
                        .map(|value| finite_clamp(value.abs(), 0.01, 100.0, 1.0));
                    let interaction_color = node.color == "$interaction";
                    let color = if interaction_color {
                        render_palette.paper
                    } else {
                        super::scene::resolve_color(palette, &node.color, render_palette.paper)
                    };
                    let mut animation = node.animation;
                    animation.orbit_radius = finite_clamp(animation.orbit_radius, 0.0, 100.0, 0.0);
                    animation.orbit_speed = finite_clamp(animation.orbit_speed, -20.0, 20.0, 0.0);
                    animation.bob_amount = finite_clamp(animation.bob_amount, -100.0, 100.0, 0.0);
                    animation.bob_speed = finite_clamp(animation.bob_speed, -20.0, 20.0, 0.0);
                    animation.pulse_amount = finite_clamp(animation.pulse_amount, 0.0, 0.95, 0.0);
                    animation.pulse_speed = finite_clamp(animation.pulse_speed, -20.0, 20.0, 0.0);
                    animation.spin_speed = finite_clamp(animation.spin_speed, -20.0, 20.0, 0.0);
                    animation.expand_amount =
                        finite_clamp(animation.expand_amount, 0.0, 100.0, 0.0);
                    animation.radial_amount =
                        finite_clamp(animation.radial_amount, -100.0, 100.0, 0.0);
                    RenderEffectNode {
                        shape: node.shape.clone(),
                        position,
                        size,
                        color,
                        interaction_color,
                        opacity: finite_clamp(node.opacity, 0.0, 1.0, 1.0),
                        count: node.count.clamp(1, MAX_EFFECT_NODE_COPIES),
                        visible_states: node
                            .visible_states
                            .iter()
                            .filter(|state| valid_effect_id(state))
                            .take(16)
                            .cloned()
                            .collect(),
                        animation,
                    }
                })
                .collect();
            (
                id.clone(),
                RenderEffectTemplate {
                    duration: finite_clamp(template.duration, 0.05, 30.0, 1.0),
                    nodes,
                },
            )
        })
        .collect()
}

pub(super) fn add_interaction(
    vertices: &mut Vec<Vertex>,
    interaction: &RenderInteraction,
    template: Option<&RenderEffectTemplate>,
    visual_state: &str,
    interaction_state: InteractionRenderState,
    elapsed: f32,
    palette: RenderPalette,
    reduced_effects: bool,
) {
    if interaction.visual.as_deref() == Some("none") {
        return;
    }
    if let Some(template) = template {
        add_template(
            vertices,
            template,
            Vec3::from_array(interaction.position),
            interaction.color,
            visual_state,
            elapsed,
            None,
            reduced_effects,
        );
    } else {
        add_default_marker(vertices, interaction, interaction_state, elapsed, palette);
    }
    super::add_pixel_text(
        vertices,
        &interaction.label,
        Vec3::from_array(interaction.position) + Vec3::new(0.0, 1.72, -0.06),
        0.0,
        3.0,
        palette.paper,
    );
}

pub(super) fn add_template(
    vertices: &mut Vec<Vertex>,
    template: &RenderEffectTemplate,
    origin: Vec3,
    interaction_color: [f32; 4],
    visual_state: &str,
    elapsed: f32,
    progress: Option<f32>,
    reduced_effects: bool,
) {
    let animation_scale = if reduced_effects { 0.2 } else { 1.0 };
    let progress = progress.map(|value| value.clamp(0.0, 1.0));
    for node in &template.nodes {
        if !node.visible_states.is_empty()
            && !node
                .visible_states
                .iter()
                .any(|state| state == visual_state)
        {
            continue;
        }
        for index in 0..node.count {
            let phase = index as f32 * std::f32::consts::TAU / node.count as f32;
            let orbit_radius = node.animation.orbit_radius
                + progress.unwrap_or(0.0) * node.animation.radial_amount;
            let orbit_angle = phase + elapsed * node.animation.orbit_speed * animation_scale;
            let mut position = Vec3::from_array(node.position);
            position.x += orbit_angle.cos() * orbit_radius;
            position.z += orbit_angle.sin() * orbit_radius;
            position.y += (elapsed * node.animation.bob_speed * animation_scale + phase).sin()
                * node.animation.bob_amount
                * animation_scale;

            let pulse = 1.0
                + (elapsed * node.animation.pulse_speed * animation_scale + phase).sin()
                    * node.animation.pulse_amount
                    * animation_scale;
            let expansion = 1.0 + progress.unwrap_or(0.0) * node.animation.expand_amount;
            let size = Vec3::from_array(node.size) * pulse.max(0.05) * expansion;
            let mut color = if node.interaction_color {
                interaction_color
            } else {
                node.color
            };
            color[3] *= node.opacity;
            if node.animation.fade {
                color[3] *= 1.0 - progress.unwrap_or(0.0);
            }

            let center = origin + position;
            match node.shape.as_str() {
                "box" => {
                    let rotation = Quat::from_rotation_y(
                        phase + elapsed * node.animation.spin_speed * animation_scale,
                    );
                    super::add_transformed_cuboid(
                        vertices,
                        Mat4::from_translation(center) * Mat4::from_quat(rotation),
                        size,
                        color,
                    );
                }
                "cylinder" => super::add_cylinder(vertices, center, size.x, size.y, color),
                "ring" => super::add_ring(vertices, center, size.x, size.y, color),
                "sphere" => super::add_sphere(vertices, center, size.x, color),
                _ => {}
            }
        }
    }
}

fn add_default_marker(
    vertices: &mut Vec<Vertex>,
    interaction: &RenderInteraction,
    state: InteractionRenderState,
    elapsed: f32,
    palette: RenderPalette,
) {
    let origin = Vec3::from_array(interaction.position);
    let pulse = 1.0 + (elapsed * 2.0).sin() * 0.025;
    super::add_cylinder(
        vertices,
        origin + Vec3::new(0.0, 0.08, 0.0),
        interaction.radius + 0.12,
        0.12,
        faded(palette.ink, 0.72),
    );
    super::add_ring(
        vertices,
        origin + Vec3::new(0.0, 0.16, 0.0),
        (interaction.radius - 0.12).max(0.35) * pulse,
        0.08,
        faded(interaction.color, if state.inside { 0.88 } else { 0.48 }),
    );
}

fn finite_clamp(value: f32, minimum: f32, maximum: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stateful_template() -> RenderEffectTemplate {
        RenderEffectTemplate {
            duration: 1.0,
            nodes: vec![RenderEffectNode {
                shape: "sphere".to_owned(),
                position: [0.0; 3],
                size: [1.0; 3],
                color: [1.0; 4],
                interaction_color: false,
                opacity: 1.0,
                count: 1,
                visible_states: vec!["open".to_owned()],
                animation: crate::effects::EffectAnimationDefinition::default(),
            }],
        }
    }

    #[test]
    fn manifest_nodes_follow_game_owned_visual_state() {
        let template = stateful_template();
        let mut hidden = Vec::new();
        add_template(
            &mut hidden,
            &template,
            Vec3::ZERO,
            [1.0; 4],
            "closed",
            0.0,
            None,
            false,
        );
        assert!(hidden.is_empty());

        let mut visible = Vec::new();
        add_template(
            &mut visible,
            &template,
            Vec3::ZERO,
            [1.0; 4],
            "open",
            0.0,
            None,
            false,
        );
        assert!(!visible.is_empty());
    }

    #[test]
    fn invisible_interactions_do_not_emit_geometry_or_labels() {
        let interaction = RenderInteraction {
            id: "trigger".to_owned(),
            label: "TRIGGER".to_owned(),
            position: [0.0; 3],
            radius: 2.0,
            color: [1.0; 4],
            visual: Some("none".to_owned()),
        };
        let mut vertices = Vec::new();
        add_interaction(
            &mut vertices,
            &interaction,
            None,
            "default",
            InteractionRenderState::default(),
            0.0,
            RenderPalette::default(),
            false,
        );
        assert!(vertices.is_empty());
    }
}
