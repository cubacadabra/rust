use super::*;

impl From<Player> for PlayerSnapshot {
    fn from(value: Player) -> Self {
        Self {
            position: value.position,
            facing_yaw: value.facing_yaw,
            velocity: value.velocity,
            grounded: value.grounded,
            climbing: value.climbing,
            moving: value.moving,
            sprinting: value.sprinting,
            walk_cycle: value.walk_cycle,
        }
    }
}
impl From<PlayerSnapshot> for Player {
    fn from(value: PlayerSnapshot) -> Self {
        Self {
            position: value.position,
            facing_yaw: value.facing_yaw,
            velocity: value.velocity,
            grounded: value.grounded,
            climbing: value.climbing,
            moving: value.moving,
            sprinting: value.sprinting,
            walk_cycle: value.walk_cycle,
        }
    }
}
impl From<Input> for InputSnapshot {
    fn from(value: Input) -> Self {
        Self {
            forward: value.forward,
            strafe: value.strafe,
            sprint: value.sprint,
            jump: value.jump,
            climb: value.climb,
            look_x: value.look_x,
            look_y: value.look_y,
            zoom_delta: value.zoom_delta,
        }
    }
}
impl From<InputSnapshot> for Input {
    fn from(value: InputSnapshot) -> Self {
        Self {
            forward: value.forward,
            strafe: value.strafe,
            sprint: value.sprint,
            jump: value.jump,
            climb: value.climb,
            look_x: value.look_x,
            look_y: value.look_y,
            zoom_delta: value.zoom_delta,
        }
    }
}
impl From<BuildBlock> for BuildBlockSnapshot {
    fn from(value: BuildBlock) -> Self {
        Self {
            position: value.position,
            size: value.size,
            color: value.color,
            rotation: value.rotation,
        }
    }
}
impl From<BuildBlockSnapshot> for BuildBlock {
    fn from(value: BuildBlockSnapshot) -> Self {
        Self {
            position: value.position,
            size: value.size,
            color: value.color,
            rotation: value.rotation % 4,
        }
    }
}

impl From<crate::world::LaunchPad> for LaunchPadSnapshot {
    fn from(value: crate::world::LaunchPad) -> Self {
        Self {
            x: value.x,
            z: value.z,
            radius: value.radius,
            countdown: value.countdown,
            phase: value.phase.code(),
            launch_at: value.launch_at,
            occupants: value.occupants as u32,
            enabled: value.enabled,
        }
    }
}
impl TryFrom<&LaunchPadSnapshot> for crate::world::LaunchPad {
    type Error = SnapshotError;
    fn try_from(value: &LaunchPadSnapshot) -> Result<Self, Self::Error> {
        let phase = match value.phase {
            0 => LaunchPadPhase::Idle,
            1 => LaunchPadPhase::Countdown,
            2 => LaunchPadPhase::Launched,
            phase => {
                return Err(SnapshotError::Invalid(format!(
                    "unknown launch-pad phase {phase}"
                )));
            }
        };
        Ok(Self {
            x: value.x,
            z: value.z,
            radius: value.radius,
            countdown: value.countdown,
            phase,
            launch_at: value.launch_at,
            occupants: value.occupants as usize,
            enabled: value.enabled,
        })
    }
}

impl From<Agent> for AgentSnapshot {
    fn from(value: Agent) -> Self {
        Self {
            position: value.position,
            target: [value.target.x, value.target.z],
            meeting_target: [value.meeting_target.x, value.meeting_target.z],
            meeting_index: value.meeting_index as u32,
            phase: match value.phase {
                AgentPhase::Entering => 0,
                AgentPhase::Roaming => 1,
                AgentPhase::Assembling => 2,
                AgentPhase::Assembled => 3,
            },
            spawned_at: value.spawned_at,
            next_decision_at: value.next_decision_at,
            gather_at: value.gather_at,
            next_jump_at: value.next_jump_at,
            speed: value.speed,
            walk_cycle: value.walk_cycle,
            vertical_velocity: value.vertical_velocity,
            grounded: value.grounded,
        }
    }
}
impl TryFrom<&AgentSnapshot> for Agent {
    type Error = SnapshotError;
    fn try_from(value: &AgentSnapshot) -> Result<Self, Self::Error> {
        let phase = match value.phase {
            0 => AgentPhase::Entering,
            1 => AgentPhase::Roaming,
            2 => AgentPhase::Assembling,
            3 => AgentPhase::Assembled,
            phase => {
                return Err(SnapshotError::Invalid(format!(
                    "unknown agent phase {phase}"
                )));
            }
        };
        Ok(Self {
            position: value.position,
            target: crate::math::Vec2 {
                x: value.target[0],
                z: value.target[1],
            },
            meeting_target: crate::math::Vec2 {
                x: value.meeting_target[0],
                z: value.meeting_target[1],
            },
            meeting_index: value.meeting_index as usize,
            phase,
            spawned_at: value.spawned_at,
            next_decision_at: value.next_decision_at,
            gather_at: value.gather_at,
            next_jump_at: value.next_jump_at,
            speed: value.speed,
            walk_cycle: value.walk_cycle,
            vertical_velocity: value.vertical_velocity,
            grounded: value.grounded,
        })
    }
}

impl From<InteractionEvent> for InteractionEventSnapshot {
    fn from(value: InteractionEvent) -> Self {
        Self {
            id: value.id,
            phase: value.phase,
            players: value.players as u32,
            position: value.position,
        }
    }
}
impl TryFrom<InteractionEventSnapshot> for InteractionEvent {
    type Error = SnapshotError;
    fn try_from(value: InteractionEventSnapshot) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            phase: value.phase,
            players: value.players as usize,
            position: value.position,
        })
    }
}

impl From<crate::types::PlayerEvent> for PlayerEventSnapshot {
    fn from(value: crate::types::PlayerEvent) -> Self {
        use crate::types::PlayerEvent::*;
        match value {
            Spawn {
                health,
                max_health,
                deaths,
            } => Self::Spawn {
                health,
                max_health,
                deaths,
            },
            Checkpoint { id, position } => Self::Checkpoint { id, position },
            Death {
                cause,
                checkpoint,
                deaths,
                health,
                max_health,
            } => Self::Death {
                cause,
                checkpoint,
                deaths,
                health,
                max_health,
            },
            Respawn {
                checkpoint,
                deaths,
                health,
                max_health,
            } => Self::Respawn {
                checkpoint,
                deaths,
                health,
                max_health,
            },
            Damage {
                source,
                amount,
                health,
                max_health,
            } => Self::Damage {
                source,
                amount,
                health,
                max_health,
            },
            Heal {
                source,
                amount,
                health,
                max_health,
            } => Self::Heal {
                source,
                amount,
                health,
                max_health,
            },
        }
    }
}
impl TryFrom<PlayerEventSnapshot> for crate::types::PlayerEvent {
    type Error = SnapshotError;
    fn try_from(value: PlayerEventSnapshot) -> Result<Self, Self::Error> {
        use PlayerEventSnapshot::*;
        Ok(match value {
            Spawn {
                health,
                max_health,
                deaths,
            } => Self::Spawn {
                health,
                max_health,
                deaths,
            },
            Checkpoint { id, position } => Self::Checkpoint { id, position },
            Death {
                cause,
                checkpoint,
                deaths,
                health,
                max_health,
            } => Self::Death {
                cause,
                checkpoint,
                deaths,
                health,
                max_health,
            },
            Respawn {
                checkpoint,
                deaths,
                health,
                max_health,
            } => Self::Respawn {
                checkpoint,
                deaths,
                health,
                max_health,
            },
            Damage {
                source,
                amount,
                health,
                max_health,
            } => Self::Damage {
                source,
                amount,
                health,
                max_health,
            },
            Heal {
                source,
                amount,
                health,
                max_health,
            } => Self::Heal {
                source,
                amount,
                health,
                max_health,
            },
        })
    }
}
