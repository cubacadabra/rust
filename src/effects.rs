use std::collections::{BTreeMap, VecDeque};

use serde::Deserialize;

pub(crate) const EFFECTS_VERSION: u16 = 1;
pub(crate) const MAX_EFFECT_TEMPLATES: usize = 64;
pub(crate) const MAX_EFFECT_NODES: usize = 32;
pub(crate) const MAX_EFFECT_NODE_COPIES: usize = 16;
pub(crate) const MAX_EFFECT_NODE_VARIANTS: usize = 16;
pub(crate) const MAX_EFFECT_COMMANDS: usize = 64;
pub(crate) const MAX_EFFECT_INSTANCES: usize = 64;
pub(crate) const MAX_EFFECT_STATES: usize = 256;

fn default_effect_version() -> u16 {
    EFFECTS_VERSION
}

fn default_effect_duration() -> f32 {
    1.0
}

fn default_effect_opacity() -> f32 {
    1.0
}

fn default_effect_count() -> usize {
    1
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffectLibraryDefinition {
    #[serde(default = "default_effect_version")]
    pub(crate) version: u16,
    #[serde(default)]
    pub(crate) templates: BTreeMap<String, EffectTemplateDefinition>,
}

impl Default for EffectLibraryDefinition {
    fn default() -> Self {
        Self {
            version: EFFECTS_VERSION,
            templates: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffectTemplateDefinition {
    #[serde(default = "default_effect_duration")]
    pub(crate) duration: f32,
    #[serde(default)]
    pub(crate) nodes: Vec<EffectNodeDefinition>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffectNodeDefinition {
    #[serde(default)]
    pub(crate) shape: String,
    #[serde(default)]
    pub(crate) position: Vec<f32>,
    #[serde(default)]
    pub(crate) size: Vec<f32>,
    #[serde(default)]
    pub(crate) color: String,
    #[serde(default = "default_effect_opacity")]
    pub(crate) opacity: f32,
    #[serde(default = "default_effect_count")]
    pub(crate) count: usize,
    #[serde(default)]
    pub(crate) visible_states: Vec<String>,
    /// Compact variants inherit this node's geometry and rendering
    /// properties while selecting their own visible states.
    #[serde(default)]
    pub(crate) variants: Vec<EffectNodeVariantDefinition>,
    #[serde(default)]
    pub(crate) animation: EffectAnimationDefinition,
}

impl EffectNodeDefinition {
    pub(crate) fn position(&self) -> [f32; 3] {
        vector3(&self.position, [0.0; 3])
    }

    pub(crate) fn size(&self) -> [f32; 3] {
        vector3(&self.size, [1.0; 3])
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffectNodeVariantDefinition {
    #[serde(default)]
    pub(crate) visible_states: Vec<String>,
    pub(crate) position: Option<Vec<f32>>,
    pub(crate) size: Option<Vec<f32>>,
    pub(crate) color: Option<String>,
    pub(crate) opacity: Option<f32>,
    pub(crate) count: Option<usize>,
    #[serde(default)]
    pub(crate) animation: EffectAnimationOverrideDefinition,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffectAnimationDefinition {
    #[serde(default)]
    pub(crate) orbit_radius: f32,
    #[serde(default)]
    pub(crate) orbit_speed: f32,
    #[serde(default)]
    pub(crate) bob_amount: f32,
    #[serde(default)]
    pub(crate) bob_speed: f32,
    #[serde(default)]
    pub(crate) pulse_amount: f32,
    #[serde(default)]
    pub(crate) pulse_speed: f32,
    #[serde(default)]
    pub(crate) spin_speed: f32,
    #[serde(default)]
    pub(crate) expand_amount: f32,
    #[serde(default)]
    pub(crate) radial_amount: f32,
    #[serde(default)]
    pub(crate) fade: bool,
}

impl EffectAnimationDefinition {
    pub(crate) fn with_override(self, value: &EffectAnimationOverrideDefinition) -> Self {
        Self {
            orbit_radius: value.orbit_radius.unwrap_or(self.orbit_radius),
            orbit_speed: value.orbit_speed.unwrap_or(self.orbit_speed),
            bob_amount: value.bob_amount.unwrap_or(self.bob_amount),
            bob_speed: value.bob_speed.unwrap_or(self.bob_speed),
            pulse_amount: value.pulse_amount.unwrap_or(self.pulse_amount),
            pulse_speed: value.pulse_speed.unwrap_or(self.pulse_speed),
            spin_speed: value.spin_speed.unwrap_or(self.spin_speed),
            expand_amount: value.expand_amount.unwrap_or(self.expand_amount),
            radial_amount: value.radial_amount.unwrap_or(self.radial_amount),
            fade: value.fade.unwrap_or(self.fade),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffectAnimationOverrideDefinition {
    pub(crate) orbit_radius: Option<f32>,
    pub(crate) orbit_speed: Option<f32>,
    pub(crate) bob_amount: Option<f32>,
    pub(crate) bob_speed: Option<f32>,
    pub(crate) pulse_amount: Option<f32>,
    pub(crate) pulse_speed: Option<f32>,
    pub(crate) spin_speed: Option<f32>,
    pub(crate) expand_amount: Option<f32>,
    pub(crate) radial_amount: Option<f32>,
    pub(crate) fade: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum EffectCommand {
    SetState {
        target: String,
        state: String,
    },
    Play {
        template: String,
        position: [f32; 3],
    },
}

#[derive(Clone, Debug)]
pub(crate) struct EffectInstance {
    pub(crate) template: String,
    pub(crate) position: [f32; 3],
    pub(crate) started_at: f32,
    pub(crate) world: usize,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct EffectRuntime {
    pub(crate) states: BTreeMap<(usize, String), String>,
    pub(crate) instances: VecDeque<EffectInstance>,
}

impl EffectRuntime {
    pub(crate) fn apply(&mut self, commands: Vec<EffectCommand>, elapsed: f32, world: usize) {
        for command in commands {
            match command {
                EffectCommand::SetState { target, state } => {
                    let key = (world, target);
                    if self.states.contains_key(&key) || self.states.len() < MAX_EFFECT_STATES {
                        self.states.insert(key, state);
                    }
                }
                EffectCommand::Play { template, position } => {
                    if self.instances.len() >= MAX_EFFECT_INSTANCES {
                        self.instances.pop_front();
                    }
                    self.instances.push_back(EffectInstance {
                        template,
                        position,
                        started_at: elapsed,
                        world,
                    });
                }
            }
        }
    }
}

pub(crate) fn valid_effect_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.is_ascii()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn vector3(values: &[f32], fallback: [f32; 3]) -> [f32; 3] {
    [
        values.first().copied().unwrap_or(fallback[0]),
        values.get(1).copied().unwrap_or(fallback[1]),
        values.get(2).copied().unwrap_or(fallback[2]),
    ]
}
