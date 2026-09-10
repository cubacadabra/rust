use serde::{Deserialize, Deserializer, Serialize, de};
use std::{collections::BTreeSet, fmt};

const MAX_CAPABILITY_ID_BYTES: usize = 64;

/// A versioned engine behavior used by one or more morph assets.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CapabilityId(String);

impl CapabilityId {
    pub fn parse(value: impl Into<String>) -> Result<Self, &'static str> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_CAPABILITY_ID_BYTES || !value.is_ascii() {
            return Err("capability ID must be 1..=64 ASCII bytes");
        }
        let Some((name, version)) = value.rsplit_once(".v") else {
            return Err("capability ID needs a .vN suffix");
        };
        if name.is_empty()
            || name.starts_with('.')
            || name.ends_with('.')
            || name.split('.').any(|segment| {
                segment.is_empty()
                    || !segment.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                    })
            })
            || version.is_empty()
            || version.starts_with('0')
            || !version.bytes().all(|byte| byte.is_ascii_digit())
            || version.parse::<u32>().is_err()
        {
            return Err("capability ID must use lower-case dot segments and a positive version");
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CapabilityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(de::Error::custom)
    }
}

/// Deterministic capability comparison shared by Studio and the runtime.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CapabilitySet(BTreeSet<CapabilityId>);

impl CapabilitySet {
    pub fn new(capabilities: impl IntoIterator<Item = CapabilityId>) -> Self {
        Self(capabilities.into_iter().collect())
    }

    pub fn contains(&self, capability: &CapabilityId) -> bool {
        self.0.contains(capability)
    }

    pub fn missing<'a>(
        &'a self,
        required: &'a [CapabilityId],
    ) -> impl Iterator<Item = &'a CapabilityId> + 'a {
        required
            .iter()
            .filter(|capability| !self.contains(capability))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_missing_capabilities_in_definition_order() {
        let rigid = CapabilityId::parse("mesh.rigid.v1").unwrap();
        let skin = CapabilityId::parse("skin.biped15-linear.v1").unwrap();
        let material = CapabilityId::parse("material.cuba-pbr.v1").unwrap();
        let supported = CapabilitySet::new([rigid.clone()]);
        let required = [rigid, skin.clone(), material.clone()];
        let missing: Vec<_> = supported.missing(&required).collect();
        assert_eq!(missing, [&skin, &material]);
    }

    #[test]
    fn rejects_unversioned_or_unstable_names() {
        assert!(CapabilityId::parse("mesh.rigid").is_err());
        assert!(CapabilityId::parse("Mesh.Rigid.v1").is_err());
        assert!(CapabilityId::parse("mesh..rigid.v1").is_err());
        assert!(CapabilityId::parse("mesh.rigid.v0").is_err());
    }
}
