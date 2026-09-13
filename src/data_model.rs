//! Generic, game-agnostic world data.
//!
//! The data model is deliberately unaware of Signal Run, Spellbound, or any
//! other game. Games provide meaning through class and property names; the
//! engine provides one mutation path and an ordered change feed that generic
//! systems can consume independently.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const ROOT_ID: u64 = 1;
const MAX_CHANGE_HISTORY: usize = 4096;
const MAX_CLASS_NAME_BYTES: usize = 64;
const MAX_ENTITY_NAME_BYTES: usize = 128;
const MAX_PROPERTY_NAME_BYTES: usize = 64;

/// Stable identity for an entity during an engine lifetime.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct EntityId(u64);

impl EntityId {
    /// The root of the engine's data model.
    pub const ROOT: Self = Self(ROOT_ID);

    /// Returns the raw identity for transport adapters and diagnostics.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Reconstructs an ID received from a persistence or networking layer.
    pub const fn from_raw(raw: u64) -> Option<Self> {
        if raw == 0 { None } else { Some(Self(raw)) }
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// The subsystem that caused a mutation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MutationSource {
    Editor,
    Script,
    Network,
    Load,
    Undo,
    Physics,
    Engine,
}

/// A serializable view of one entity in the model.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EntitySnapshot {
    pub id: EntityId,
    pub class: String,
    pub name: String,
    pub parent: Option<EntityId>,
    pub properties: BTreeMap<String, Value>,
}

/// The semantic part of one mutation. The sequence and source live on the
/// surrounding [`DataModelChange`] envelope so every event has the same shape.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum DataModelEvent {
    EntityCreated {
        entity: EntityId,
        class: String,
        name: String,
        parent: Option<EntityId>,
    },
    EntityDestroyed {
        entity: EntityId,
        snapshot: EntitySnapshot,
    },
    ParentChanged {
        entity: EntityId,
        previous: Option<EntityId>,
        parent: Option<EntityId>,
    },
    NameChanged {
        entity: EntityId,
        previous: String,
        name: String,
    },
    PropertyChanged {
        entity: EntityId,
        property: String,
        previous: Option<Value>,
        value: Value,
    },
    PropertyRemoved {
        entity: EntityId,
        property: String,
        previous: Value,
    },
}

/// One ordered mutation in the model's change feed.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DataModelChange {
    pub sequence: u64,
    pub source: MutationSource,
    pub event: DataModelEvent,
}

/// A consumer's position in the ordered change feed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataModelCursor {
    next_sequence: u64,
}

/// Errors returned by the central mutation API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataModelError {
    EntityNotFound(EntityId),
    ParentNotFound(EntityId),
    RootCannotBeReparented,
    RootCannotBeDestroyed,
    ParentCycle,
    InvalidClassName,
    InvalidEntityName,
    InvalidPropertyName,
    CursorTooOld { requested: u64, oldest: u64 },
}

impl fmt::Display for DataModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EntityNotFound(entity) => write!(formatter, "entity {entity} was not found"),
            Self::ParentNotFound(entity) => {
                write!(formatter, "parent entity {entity} was not found")
            }
            Self::RootCannotBeReparented => {
                formatter.write_str("the data-model root cannot be reparented")
            }
            Self::RootCannotBeDestroyed => {
                formatter.write_str("the data-model root cannot be destroyed")
            }
            Self::ParentCycle => formatter.write_str("parenting the entity would create a cycle"),
            Self::InvalidClassName => formatter.write_str("class name is empty or too long"),
            Self::InvalidEntityName => formatter.write_str("entity name is empty or too long"),
            Self::InvalidPropertyName => formatter.write_str("property name is empty or too long"),
            Self::CursorTooOld { requested, oldest } => write!(
                formatter,
                "change cursor starts at sequence {requested}, but the oldest retained change is {oldest}"
            ),
        }
    }
}

impl std::error::Error for DataModelError {}

#[derive(Clone, Debug)]
struct Entity {
    class: String,
    name: String,
    parent: Option<EntityId>,
    properties: BTreeMap<String, Value>,
}

/// The generic, authoritative object/property graph owned by the engine.
pub struct DataModel {
    entities: BTreeMap<EntityId, Entity>,
    next_entity: u64,
    next_sequence: u64,
    changes: VecDeque<DataModelChange>,
}

impl Default for DataModel {
    fn default() -> Self {
        Self::new()
    }
}

impl DataModel {
    /// Creates a model with a stable `game`-style root and no mutation event
    /// for bootstrap. Loading and game-authored entities use the same APIs
    /// afterward, so their changes are observable in exactly the same way.
    pub fn new() -> Self {
        let mut entities = BTreeMap::new();
        entities.insert(
            EntityId::ROOT,
            Entity {
                class: "DataModel".to_owned(),
                name: "game".to_owned(),
                parent: None,
                properties: BTreeMap::new(),
            },
        );
        Self {
            entities,
            next_entity: ROOT_ID + 1,
            next_sequence: 1,
            changes: VecDeque::new(),
        }
    }

    pub fn root(&self) -> EntityId {
        EntityId::ROOT
    }

    pub fn contains(&self, entity: EntityId) -> bool {
        self.entities.contains_key(&entity)
    }

    pub fn entity(&self, entity: EntityId) -> Result<EntitySnapshot, DataModelError> {
        self.entities
            .get(&entity)
            .map(|value| self.snapshot(entity, value))
            .ok_or(DataModelError::EntityNotFound(entity))
    }

    pub fn children(&self, parent: EntityId) -> Result<Vec<EntityId>, DataModelError> {
        if !self.entities.contains_key(&parent) {
            return Err(DataModelError::EntityNotFound(parent));
        }
        Ok(self
            .entities
            .iter()
            .filter_map(|(entity, value)| (value.parent == Some(parent)).then_some(*entity))
            .collect())
    }

    pub fn descendants(&self, entity: EntityId) -> Result<Vec<EntityId>, DataModelError> {
        if !self.entities.contains_key(&entity) {
            return Err(DataModelError::EntityNotFound(entity));
        }
        let mut descendants = Vec::new();
        for child in self.children(entity)? {
            self.collect_subtree(child, &mut descendants);
        }
        Ok(descendants)
    }

    pub fn snapshot_state(&self) -> Vec<EntitySnapshot> {
        self.entities
            .iter()
            .map(|(id, entity)| self.snapshot(*id, entity))
            .collect()
    }

    pub fn get_property(
        &self,
        entity: EntityId,
        property: &str,
    ) -> Result<Option<&Value>, DataModelError> {
        let entity = self
            .entities
            .get(&entity)
            .ok_or(DataModelError::EntityNotFound(entity))?;
        Ok(entity.properties.get(property))
    }

    pub fn create_entity(
        &mut self,
        class: impl Into<String>,
        name: impl Into<String>,
        parent: Option<EntityId>,
        source: MutationSource,
    ) -> Result<EntityId, DataModelError> {
        let class = class.into();
        let name = name.into();
        validate_class_name(&class)?;
        validate_entity_name(&name)?;
        if let Some(parent) = parent
            && !self.entities.contains_key(&parent)
        {
            return Err(DataModelError::ParentNotFound(parent));
        }

        let entity = EntityId(self.next_entity);
        self.next_entity = self.next_entity.saturating_add(1);
        self.entities.insert(
            entity,
            Entity {
                class: class.clone(),
                name: name.clone(),
                parent,
                properties: BTreeMap::new(),
            },
        );
        self.publish(
            source,
            DataModelEvent::EntityCreated {
                entity,
                class,
                name,
                parent,
            },
        );
        Ok(entity)
    }

    pub fn set_name(
        &mut self,
        entity: EntityId,
        name: impl Into<String>,
        source: MutationSource,
    ) -> Result<bool, DataModelError> {
        let name = name.into();
        validate_entity_name(&name)?;
        let entity_data = self
            .entities
            .get_mut(&entity)
            .ok_or(DataModelError::EntityNotFound(entity))?;
        if entity_data.name == name {
            return Ok(false);
        }
        let previous = std::mem::replace(&mut entity_data.name, name.clone());
        self.publish(
            source,
            DataModelEvent::NameChanged {
                entity,
                previous,
                name,
            },
        );
        Ok(true)
    }

    /// Sets a property through the one mutation path. Returns `false` when
    /// the value was already equal, which intentionally produces no event.
    pub fn set_property(
        &mut self,
        entity: EntityId,
        property: impl Into<String>,
        value: Value,
        source: MutationSource,
    ) -> Result<bool, DataModelError> {
        let property = property.into();
        validate_property_name(&property)?;
        let entity_data = self
            .entities
            .get_mut(&entity)
            .ok_or(DataModelError::EntityNotFound(entity))?;
        if entity_data.properties.get(&property) == Some(&value) {
            return Ok(false);
        }
        let previous = entity_data
            .properties
            .insert(property.clone(), value.clone());
        self.publish(
            source,
            DataModelEvent::PropertyChanged {
                entity,
                property,
                previous,
                value,
            },
        );
        Ok(true)
    }

    pub fn remove_property(
        &mut self,
        entity: EntityId,
        property: impl Into<String>,
        source: MutationSource,
    ) -> Result<bool, DataModelError> {
        let property = property.into();
        validate_property_name(&property)?;
        let entity_data = self
            .entities
            .get_mut(&entity)
            .ok_or(DataModelError::EntityNotFound(entity))?;
        let Some(previous) = entity_data.properties.remove(&property) else {
            return Ok(false);
        };
        self.publish(
            source,
            DataModelEvent::PropertyRemoved {
                entity,
                property,
                previous,
            },
        );
        Ok(true)
    }

    pub fn set_parent(
        &mut self,
        entity: EntityId,
        parent: Option<EntityId>,
        source: MutationSource,
    ) -> Result<bool, DataModelError> {
        if entity == EntityId::ROOT {
            return Err(DataModelError::RootCannotBeReparented);
        }
        if !self.entities.contains_key(&entity) {
            return Err(DataModelError::EntityNotFound(entity));
        }
        if let Some(parent) = parent {
            if !self.entities.contains_key(&parent) {
                return Err(DataModelError::ParentNotFound(parent));
            }
            if parent == entity || self.is_descendant_of(parent, entity) {
                return Err(DataModelError::ParentCycle);
            }
        }
        let previous = self.entities.get(&entity).and_then(|value| value.parent);
        if previous == parent {
            return Ok(false);
        }
        self.entities
            .get_mut(&entity)
            .expect("entity was checked above")
            .parent = parent;
        self.publish(
            source,
            DataModelEvent::ParentChanged {
                entity,
                previous,
                parent,
            },
        );
        Ok(true)
    }

    /// Destroys an entity and its descendants. Destruction events contain a
    /// pre-despawn snapshot so consumers can replicate, undo, or clean up
    /// references without racing the removal.
    pub fn destroy(
        &mut self,
        entity: EntityId,
        source: MutationSource,
    ) -> Result<Vec<EntityId>, DataModelError> {
        if entity == EntityId::ROOT {
            return Err(DataModelError::RootCannotBeDestroyed);
        }
        if !self.entities.contains_key(&entity) {
            return Err(DataModelError::EntityNotFound(entity));
        }
        let mut subtree = Vec::new();
        self.collect_subtree(entity, &mut subtree);
        for descendant in &subtree {
            let snapshot = self.entity(*descendant)?;
            self.publish(
                source,
                DataModelEvent::EntityDestroyed {
                    entity: *descendant,
                    snapshot,
                },
            );
        }
        for descendant in &subtree {
            self.entities.remove(descendant);
        }
        Ok(subtree)
    }

    /// Starts a consumer at the current end of the feed.
    pub fn subscribe(&self) -> DataModelCursor {
        DataModelCursor {
            next_sequence: self.next_sequence,
        }
    }

    /// Starts a consumer at the oldest retained change.
    pub fn subscribe_from_start(&self) -> DataModelCursor {
        DataModelCursor {
            next_sequence: self
                .changes
                .front()
                .map_or(self.next_sequence, |change| change.sequence),
        }
    }

    /// Returns each change after the cursor and advances that cursor. The
    /// bounded history makes a stalled consumer detectable instead of silently
    /// delivering an incomplete view.
    pub fn changes_since(
        &self,
        cursor: &mut DataModelCursor,
    ) -> Result<Vec<DataModelChange>, DataModelError> {
        if let Some(oldest) = self.changes.front().map(|change| change.sequence)
            && cursor.next_sequence < oldest
        {
            return Err(DataModelError::CursorTooOld {
                requested: cursor.next_sequence,
                oldest,
            });
        }
        let changes = self
            .changes
            .iter()
            .filter(|change| change.sequence >= cursor.next_sequence)
            .cloned()
            .collect();
        cursor.next_sequence = self.next_sequence;
        Ok(changes)
    }

    pub fn mutation_count(&self) -> u64 {
        self.next_sequence.saturating_sub(1)
    }

    fn publish(&mut self, source: MutationSource, event: DataModelEvent) {
        let change = DataModelChange {
            sequence: self.next_sequence,
            source,
            event,
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.changes.push_back(change);
        while self.changes.len() > MAX_CHANGE_HISTORY {
            self.changes.pop_front();
        }
    }

    fn snapshot(&self, id: EntityId, entity: &Entity) -> EntitySnapshot {
        EntitySnapshot {
            id,
            class: entity.class.clone(),
            name: entity.name.clone(),
            parent: entity.parent,
            properties: entity.properties.clone(),
        }
    }

    fn is_descendant_of(&self, entity: EntityId, ancestor: EntityId) -> bool {
        let mut current = Some(entity);
        while let Some(candidate) = current {
            if candidate == ancestor {
                return true;
            }
            current = self.entities.get(&candidate).and_then(|value| value.parent);
        }
        false
    }

    fn collect_subtree(&self, entity: EntityId, output: &mut Vec<EntityId>) {
        output.push(entity);
        for child in self
            .entities
            .iter()
            .filter_map(|(id, value)| (value.parent == Some(entity)).then_some(*id))
        {
            self.collect_subtree(child, output);
        }
    }
}

fn validate_class_name(value: &str) -> Result<(), DataModelError> {
    (!value.is_empty() && value.len() <= MAX_CLASS_NAME_BYTES)
        .then_some(())
        .ok_or(DataModelError::InvalidClassName)
}

fn validate_entity_name(value: &str) -> Result<(), DataModelError> {
    (!value.is_empty() && value.len() <= MAX_ENTITY_NAME_BYTES)
        .then_some(())
        .ok_or(DataModelError::InvalidEntityName)
}

fn validate_property_name(value: &str) -> Result<(), DataModelError> {
    (!value.is_empty() && value.len() <= MAX_PROPERTY_NAME_BYTES)
        .then_some(())
        .ok_or(DataModelError::InvalidPropertyName)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{DataModel, DataModelError, DataModelEvent, MutationSource};

    #[test]
    fn mutations_are_centralized_and_noops_do_not_publish() {
        let mut model = DataModel::new();
        let entity = model
            .create_entity(
                "Beacon",
                "Node 1",
                Some(model.root()),
                MutationSource::Script,
            )
            .expect("entity should be created");
        assert!(
            model
                .set_property(entity, "team", json!("red"), MutationSource::Script)
                .unwrap()
        );
        assert!(
            !model
                .set_property(entity, "team", json!("red"), MutationSource::Script)
                .unwrap()
        );

        let mut cursor = model.subscribe_from_start();
        let changes = model
            .changes_since(&mut cursor)
            .expect("cursor should be current");
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].source, MutationSource::Script);
        assert!(matches!(
            changes[1].event,
            DataModelEvent::PropertyChanged { ref property, .. } if property == "team"
        ));
    }

    #[test]
    fn consumers_have_independent_cursors() {
        let mut model = DataModel::new();
        let mut first = model.subscribe_from_start();
        let mut second = model.subscribe_from_start();
        model
            .create_entity("Part", "Beacon", Some(model.root()), MutationSource::Engine)
            .expect("entity should be created");

        assert_eq!(model.changes_since(&mut first).unwrap().len(), 1);
        assert_eq!(model.changes_since(&mut second).unwrap().len(), 1);
        assert!(model.changes_since(&mut first).unwrap().is_empty());
    }

    #[test]
    fn names_and_snapshots_are_part_of_the_generic_model() {
        let mut model = DataModel::new();
        let entity = model
            .create_entity("Part", "Old", Some(model.root()), MutationSource::Engine)
            .unwrap();
        assert!(
            model
                .set_name(entity, "New", MutationSource::Editor)
                .unwrap()
        );
        assert_eq!(model.entity(entity).unwrap().name, "New");
        assert_eq!(model.snapshot_state().len(), 2);
        assert!(matches!(
            model
                .changes_since(&mut model.subscribe_from_start())
                .unwrap()
                .last()
                .map(|change| &change.event),
            Some(DataModelEvent::NameChanged { name, .. }) if name == "New"
        ));
    }

    #[test]
    fn hierarchy_is_validated_and_destroy_events_keep_snapshots() {
        let mut model = DataModel::new();
        let parent = model
            .create_entity("Model", "Parent", Some(model.root()), MutationSource::Load)
            .unwrap();
        let child = model
            .create_entity("Part", "Child", Some(parent), MutationSource::Load)
            .unwrap();
        assert_eq!(
            model.set_parent(parent, Some(child), MutationSource::Editor),
            Err(DataModelError::ParentCycle)
        );
        let destroyed = model.destroy(parent, MutationSource::Undo).unwrap();
        assert_eq!(destroyed, vec![parent, child]);
        let mut cursor = model.subscribe_from_start();
        let changes = model.changes_since(&mut cursor).unwrap();
        assert!(matches!(
            changes.last().map(|change| &change.event),
            Some(DataModelEvent::EntityDestroyed { snapshot, .. })
                if snapshot.name == "Child"
        ));
    }

    #[test]
    fn stalled_cursors_report_history_loss() {
        let mut model = DataModel::new();
        let mut cursor = model.subscribe_from_start();
        for index in 0..4100 {
            model
                .create_entity(
                    "Part",
                    format!("Part {index}"),
                    Some(model.root()),
                    MutationSource::Engine,
                )
                .unwrap();
        }
        assert!(matches!(
            model.changes_since(&mut cursor),
            Err(DataModelError::CursorTooOld { .. })
        ));
    }
}
