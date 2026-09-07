use crate::engine::Engine;
use crate::game_package::InteractionDefinition;
use crate::math::horizontal_distance;
use crate::scripting::{InteractionScriptState, InteractionZoneState};
use crate::types::{InteractionEvent, InteractionRenderState};
use crate::world::InteractionZone;

const MAX_INTERACTIONS: usize = 128;
const NEARBY_PADDING: f32 = 3.0;

#[derive(Clone, Debug, Default)]
pub(crate) struct InteractionRuntime {
    pub(crate) world: Vec<InteractionZone>,
    states: Vec<InteractionRenderState>,
    was_inside: Vec<bool>,
    events: Vec<InteractionEvent>,
    event_id: u32,
}

impl InteractionRuntime {
    pub(crate) fn from_definitions(definitions: &[InteractionDefinition]) -> Self {
        let world = definitions
            .iter()
            .filter(|definition| !definition.id.trim().is_empty())
            .take(MAX_INTERACTIONS)
            .map(|definition| InteractionZone {
                id: definition.id.trim().to_owned(),
                label: if definition.label.trim().is_empty() {
                    definition.id.trim().to_owned()
                } else {
                    definition.label.clone()
                },
                kind: if definition.kind.trim().is_empty() {
                    "zone".to_owned()
                } else {
                    definition.kind.clone()
                },
                position: definition.position(),
                radius: definition.radius.max(0.5),
            })
            .collect::<Vec<_>>();
        Self {
            states: vec![InteractionRenderState::default(); world.len()],
            was_inside: vec![false; world.len()],
            world,
            ..Self::default()
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn state(&self, index: usize) -> InteractionRenderState {
        self.states.get(index).copied().unwrap_or_default()
    }

    pub(crate) fn take_events(&mut self) -> Vec<InteractionEvent> {
        std::mem::take(&mut self.events)
    }
}

impl Engine {
    pub(crate) fn set_interaction_world(&mut self, zones: Vec<InteractionZone>) {
        self.interactions.clear();
        self.interactions.world = zones;
        self.interactions.states =
            vec![InteractionRenderState::default(); self.interactions.world.len()];
        self.interactions.was_inside = vec![false; self.interactions.world.len()];
    }

    pub(crate) fn interaction_render_state(&self, index: usize) -> InteractionRenderState {
        self.interactions.state(index)
    }

    pub(crate) fn interaction_count(&self) -> usize {
        self.interactions.world.len()
    }

    pub(crate) fn update_interactions(&mut self) {
        let zones = self.interactions.world.clone();
        for (index, zone) in zones.iter().enumerate() {
            let distance = horizontal_distance(
                self.player.position[0],
                self.player.position[2],
                zone.position[0],
                zone.position[2],
            );
            let inside = distance <= zone.radius;
            let players = self.players_near(zone.position, zone.radius);
            let was_inside = self.interactions.was_inside[index];
            self.interactions.states[index] = InteractionRenderState {
                inside,
                players,
                event_id: self.interactions.event_id,
            };
            if inside != was_inside {
                self.interactions.event_id = self.interactions.event_id.wrapping_add(1);
                self.interactions.states[index].event_id = self.interactions.event_id;
                self.interactions.events.push(InteractionEvent {
                    id: zone.id.clone(),
                    phase: if inside { "enter" } else { "exit" }.to_owned(),
                    players,
                });
                self.interactions.was_inside[index] = inside;
            }
        }
    }

    pub(crate) fn sync_interaction_script_state(&self) {
        let Some(script) = &self.script else {
            return;
        };
        let zones = self
            .interactions
            .world
            .iter()
            .enumerate()
            .map(|(index, zone)| {
                let state = self.interactions.state(index);
                let distance = horizontal_distance(
                    self.player.position[0],
                    self.player.position[2],
                    zone.position[0],
                    zone.position[2],
                );
                InteractionZoneState {
                    id: zone.id.clone(),
                    kind: zone.kind.clone(),
                    label: zone.label.clone(),
                    inside: state.inside,
                    nearby: distance <= zone.radius + NEARBY_PADDING,
                    players: state.players,
                }
            })
            .collect();
        script.set_interaction_state(InteractionScriptState {
            zones,
            event_id: self.interactions.event_id,
        });
    }

    pub(crate) fn take_interaction_events(&mut self) -> Vec<InteractionEvent> {
        self.interactions.take_events()
    }

    fn players_near(&self, position: [f32; 3], radius: f32) -> usize {
        usize::from(self.actor_is_near_local(position, radius))
            + self
                .remote_players
                .iter()
                .filter(|_| self.remote_world_matches_active())
                .filter(|player| {
                    horizontal_distance(
                        player.position[0],
                        player.position[2],
                        position[0],
                        position[2],
                    ) <= radius
                })
                .count()
    }

    fn actor_is_near_local(&self, position: [f32; 3], radius: f32) -> bool {
        horizontal_distance(
            self.player.position[0],
            self.player.position[2],
            position[0],
            position[2],
        ) <= radius
    }
}
