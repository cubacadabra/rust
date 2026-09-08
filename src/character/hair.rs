//! Bounded hairstyle authoring data, independent of the renderer and rig.
use super::BodyId;
use serde::Deserialize;
use std::sync::OnceLock;

pub(crate) const MAX_LOCKS: usize = 20;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HairStyle {
    pub(crate) color: [f32; 3],
    pub(crate) cap: HairCap,
    pub(crate) locks: Vec<HairLock>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HairCap {
    pub(crate) center: [f32; 3],
    pub(crate) size: [f32; 3],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HairLock {
    // Cubic Bezier: root, two controls, tip, in head-local coordinates.
    pub(crate) points: [[f32; 3]; 4],
    pub(crate) width: f32,
    pub(crate) depth: f32,
    pub(crate) outward: [f32; 3],
    // Relative cross-section radii at evenly spaced points along the curve.
    pub(crate) profile: [f32; 6],
    pub(crate) sway: f32,
}

const SOURCES: [(&str, &str); 2] = [
    (
        "ponytail.json",
        include_str!("../../assets/characters/person/hair/ponytail.json"),
    ),
    (
        "shag.json",
        include_str!("../../assets/characters/person/hair/shag.json"),
    ),
];

pub(crate) fn for_body(body: BodyId) -> Option<(u8, &'static HairStyle)> {
    let index = match body {
        BodyId::PersonGirl => 0,
        BodyId::PersonNonbinary => 1,
        _ => return None,
    };
    Some((index, get(index)))
}

pub(crate) fn get(index: u8) -> &'static HairStyle {
    static STYLES: OnceLock<[HairStyle; 2]> = OnceLock::new();
    &STYLES.get_or_init(|| SOURCES.map(|(name, source)| load(name, source)))[index as usize]
}

fn parse(source: &str) -> Result<HairStyle, String> {
    if source.len() > 64 * 1024 {
        return Err("hairstyle exceeds 64 KiB".into());
    }
    let style: HairStyle = serde_json::from_str(source).map_err(|error| error.to_string())?;
    let position_ok = |p: &[f32; 3]| p.iter().all(|v| v.is_finite() && v.abs() <= 2.0);
    if !style.color.iter().all(|v| (0.0..=1.0).contains(v))
        || !position_ok(&style.cap.center)
        || !style.cap.size.iter().all(|v| (0.01..=2.0).contains(v))
        || style.locks.is_empty()
        || style.locks.len() > MAX_LOCKS
        || style.locks.iter().any(|lock| {
            !lock.points.iter().all(position_ok)
                || lock.points.windows(2).any(|pair| pair[0] == pair[1])
                || !(0.01..=1.0).contains(&lock.width)
                || !(0.01..=1.0).contains(&lock.depth)
                || !position_ok(&lock.outward)
                || glam::Vec3::from_array(lock.outward).length_squared() < 0.01
                || !lock.profile.iter().all(|v| (0.005..=0.6).contains(v))
                || !(0.0..=1.0).contains(&lock.sway)
        })
    {
        return Err("invalid hairstyle dimensions, color, curve, or lock count".into());
    }
    Ok(style)
}

fn load(_name: &str, source: &str) -> HairStyle {
    // Restart a native development process to read edits without rebuilding.
    // Packaged and WASM builds always have the embedded fallback available.
    #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
    {
        let directory = std::env::var_os("CUBACADABRA_HAIR_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/assets/characters/person/hair"
                ))
            });
        let path = directory.join(_name);
        match std::fs::read_to_string(&path) {
            Ok(text) => match parse(&text) {
                Ok(style) => return style,
                Err(error) => {
                    log::warn!("Hair asset {}: {error}; using bundled data", path.display())
                }
            },
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                log::warn!("Hair asset {}: {error}; using bundled data", path.display());
            }
            Err(_) => {}
        }
    }
    parse(source).expect("bundled hairstyle must be valid")
}
