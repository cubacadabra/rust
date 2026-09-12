use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The small, engine-owned vocabulary used to describe reflected properties.
///
/// This is intentionally narrower than a general reflection system. The
/// runtime only needs stable property identity, a value shape, defaults, and
/// bounded validation for the first label-maker slice.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PropertyValueType {
    String,
    Number,
    Vector3,
}

impl PropertyValueType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Vector3 => "vector3",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "value")]
pub(crate) enum PropertyValue {
    String(String),
    Number(f32),
    Vector3([f32; 3]),
}

impl PropertyValue {
    pub(crate) fn value_type(&self) -> PropertyValueType {
        match self {
            Self::String(_) => PropertyValueType::String,
            Self::Number(_) => PropertyValueType::Number,
            Self::Vector3(_) => PropertyValueType::Vector3,
        }
    }

    pub(crate) fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn as_number(&self) -> Option<f32> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub(crate) fn as_vector3(&self) -> Option<[f32; 3]> {
        match self {
            Self::Vector3(value) => Some(*value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PropertyRange {
    pub(crate) min: Option<f32>,
    pub(crate) max: Option<f32>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PropertyFlags {
    pub(crate) script_visible: bool,
    pub(crate) serializable: bool,
    pub(crate) editor_visible: bool,
    pub(crate) replicable: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PropertySchema {
    /// Stable machine identity. This must not be a display label.
    pub(crate) id: String,
    /// Human-readable name shared by tools and documentation.
    pub(crate) name: String,
    pub(crate) value_type: PropertyValueType,
    pub(crate) default: PropertyValue,
    #[serde(default)]
    pub(crate) range: PropertyRange,
    #[serde(default)]
    pub(crate) flags: PropertyFlags,
}

impl PropertySchema {
    fn new(
        id: &str,
        name: &str,
        default: PropertyValue,
        range: PropertyRange,
        flags: PropertyFlags,
    ) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            value_type: default.value_type(),
            default,
            range,
            flags,
        }
    }

    pub(crate) fn validate(&self, value: &PropertyValue) -> Result<(), String> {
        if value.value_type() != self.value_type {
            return Err(format!(
                "property {} expects {}, received {}",
                self.id,
                self.value_type.as_str(),
                value.value_type().as_str()
            ));
        }

        match value {
            PropertyValue::Number(value) => {
                if !value.is_finite() {
                    return Err(format!("property {} must be finite", self.id));
                }
                if self.range.min.is_some_and(|min| *value < min) {
                    return Err(format!("property {} is below its minimum", self.id));
                }
                if self.range.max.is_some_and(|max| *value > max) {
                    return Err(format!("property {} is above its maximum", self.id));
                }
            }
            PropertyValue::Vector3(values) => {
                if values.iter().any(|value| !value.is_finite()) {
                    return Err(format!("property {} must contain finite values", self.id));
                }
            }
            PropertyValue::String(_) => {}
        }
        Ok(())
    }

    pub(crate) fn sanitize(&self, value: PropertyValue) -> PropertyValue {
        if self.validate(&value).is_ok() {
            return value;
        }

        match value {
            PropertyValue::Number(value) if value.is_finite() => {
                let value = self.range.min.map_or(value, |min| value.max(min));
                let value = self.range.max.map_or(value, |max| value.min(max));
                PropertyValue::Number(value)
            }
            _ => self.default.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClassSchema {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) properties: Vec<PropertySchema>,
}

impl ClassSchema {
    pub(crate) fn property(&self, id: &str) -> Option<&PropertySchema> {
        self.properties.iter().find(|property| property.id == id)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ClassRegistry {
    classes: BTreeMap<String, ClassSchema>,
}

impl ClassRegistry {
    pub(crate) fn register(&mut self, class: ClassSchema) -> Result<(), String> {
        if self.classes.contains_key(&class.id) {
            return Err(format!("duplicate class id: {}", class.id));
        }

        let mut property_ids = BTreeMap::new();
        for property in &class.properties {
            if property_ids.insert(property.id.clone(), ()).is_some() {
                return Err(format!(
                    "duplicate property id {} in class {}",
                    property.id, class.id
                ));
            }
            property.validate(&property.default)?;
        }

        self.classes.insert(class.id.clone(), class);
        Ok(())
    }

    pub(crate) fn class(&self, id: &str) -> Option<&ClassSchema> {
        self.classes.get(id)
    }
}

pub(crate) const INTERACTION_ZONE_CLASS_ID: &str = "cuba:interaction-zone.v1";
pub(crate) const INTERACTION_ZONE_LABEL_ID: &str = "cuba:interaction-zone.label";
pub(crate) const INTERACTION_ZONE_KIND_ID: &str = "cuba:interaction-zone.kind";
pub(crate) const INTERACTION_ZONE_POSITION_ID: &str = "cuba:interaction-zone.position";
pub(crate) const INTERACTION_ZONE_RADIUS_ID: &str = "cuba:interaction-zone.radius";

pub(crate) fn interaction_zone_registry() -> ClassRegistry {
    let mut registry = ClassRegistry::default();
    registry
        .register(interaction_zone_schema())
        .expect("built-in interaction zone schema must be valid");
    registry
}

fn interaction_zone_schema() -> ClassSchema {
    let runtime_flags = |replicable| PropertyFlags {
        script_visible: true,
        serializable: true,
        editor_visible: true,
        replicable,
    };

    ClassSchema {
        id: INTERACTION_ZONE_CLASS_ID.to_owned(),
        name: "InteractionZone".to_owned(),
        properties: vec![
            PropertySchema::new(
                INTERACTION_ZONE_LABEL_ID,
                "Label",
                PropertyValue::String(String::new()),
                PropertyRange::default(),
                runtime_flags(false),
            ),
            PropertySchema::new(
                INTERACTION_ZONE_KIND_ID,
                "Kind",
                PropertyValue::String("zone".to_owned()),
                PropertyRange::default(),
                runtime_flags(false),
            ),
            PropertySchema::new(
                INTERACTION_ZONE_POSITION_ID,
                "Position",
                PropertyValue::Vector3([0.0, 0.0, 0.0]),
                PropertyRange::default(),
                runtime_flags(true),
            ),
            PropertySchema::new(
                INTERACTION_ZONE_RADIUS_ID,
                "Radius",
                PropertyValue::Number(2.7),
                PropertyRange {
                    min: Some(0.5),
                    max: Some(128.0),
                },
                runtime_flags(true),
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_property(id: &str) -> PropertySchema {
        PropertySchema::new(
            id,
            "Test property",
            PropertyValue::Number(1.0),
            PropertyRange {
                min: Some(0.0),
                max: Some(2.0),
            },
            PropertyFlags::default(),
        )
    }

    #[test]
    fn registry_rejects_duplicate_property_ids() {
        let mut registry = ClassRegistry::default();
        let class = ClassSchema {
            id: "test:class.v1".to_owned(),
            name: "Test".to_owned(),
            properties: vec![test_property("test:value"), test_property("test:value")],
        };

        assert_eq!(
            registry.register(class).unwrap_err(),
            "duplicate property id test:value in class test:class.v1"
        );
    }

    #[test]
    fn property_validation_enforces_type_and_range() {
        let property = test_property("test:value");

        assert!(property.validate(&PropertyValue::Number(1.5)).is_ok());
        assert!(property.validate(&PropertyValue::Number(-1.0)).is_err());
        assert!(
            property
                .validate(&PropertyValue::String("wrong".to_owned()))
                .is_err()
        );
        assert_eq!(
            property.sanitize(PropertyValue::Number(-1.0)),
            PropertyValue::Number(0.0)
        );
    }

    #[test]
    fn property_schema_serializes_and_round_trips() {
        let registry = interaction_zone_registry();
        let property = registry
            .class(INTERACTION_ZONE_CLASS_ID)
            .and_then(|class| class.property(INTERACTION_ZONE_RADIUS_ID))
            .expect("built-in radius property should exist");
        let encoded = serde_json::to_string(property).expect("schema should serialize");
        let decoded: PropertySchema =
            serde_json::from_str(&encoded).expect("schema should deserialize");

        assert_eq!(&decoded, property);
    }
}
