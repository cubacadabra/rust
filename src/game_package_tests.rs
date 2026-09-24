use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_external_game_manifest_parses() {
        let Ok(path) = std::env::var("CUBACADABRA_TEST_GAME_MANIFEST") else {
            return;
        };
        let source = std::fs::read_to_string(&path).expect("configured game manifest should exist");
        GamePackageDefinition::parse(&source).expect("configured game manifest should parse");
    }

    #[test]
    fn rejects_an_unsupported_sdk_version() {
        let error = GamePackageDefinition::parse(r#"{"sdkVersion":"0.7.0"}"#)
            .expect_err("unsupported SDK versions must be rejected");
        assert!(error.to_string().contains("unsupported sdkVersion"));
    }

    #[test]
    fn accepts_terrain_sdk_version_and_requires_it_for_terrain_data() {
        let supported = GamePackageDefinition::parse(r#"{"sdkVersion":"0.4.0"}"#)
            .expect("terrain-capable SDK should be accepted");
        assert_eq!(supported._sdk_version.as_deref(), Some(TERRAIN_SDK_VERSION));
        let legacy = GamePackageDefinition::parse(r#"{"sdkVersion":"0.5.0"}"#)
            .expect("legacy current SDK should be accepted");
        assert_eq!(
            legacy._sdk_version.as_deref(),
            Some(LEGACY_CURRENT_SDK_VERSION)
        );
        let current = GamePackageDefinition::parse(r#"{"sdkVersion":"0.6.0"}"#)
            .expect("current SDK should be accepted");
        assert_eq!(current._sdk_version.as_deref(), Some(CURRENT_SDK_VERSION));

        let scale3 = r#"{
            "sdkVersion":"0.5.0",
            "worlds":{"world":{"decorations":[{"asset":"chair","scale3":[1,2,3]}]}}
        }"#;
        let error = GamePackageDefinition::parse(scale3)
            .expect_err("non-uniform scale must require its capability SDK");
        assert!(
            error
                .to_string()
                .contains("mesh scale3 requires sdkVersion 0.6.0")
        );
        let malformed_scale3 = r#"{
            "sdkVersion":"0.6.0",
            "worlds":{"world":{"decorations":[{"asset":"chair","scale3":[1,2]}]}}
        }"#;
        GamePackageDefinition::parse(malformed_scale3)
            .expect_err("scale3 must contain exactly three values");

        let top_level = r##"{
            "sdkVersion":"0.4.0",
            "terrain":{"operations":[{
                "shape":"block","operation":"fill","position":[0,0,0],
                "size":[2,2,2],"material":"builtin:grass"
            }]}
        }"##;
        let package = GamePackageDefinition::parse(top_level)
            .expect("top-level terrain should be supported for the lobby world");
        assert_eq!(package.world_entries()[0].1.terrain.operations.len(), 1);

        let terrain = r##"{
            "sdkVersion":"0.3.0",
            "worlds":{"maze":{"terrain":{"operations":[{
                "shape":"block","operation":"fill","position":[0,0,0],
                "size":[2,2,2],"material":"builtin:grass"
            }]}}}
        }"##;
        let error = GamePackageDefinition::parse(terrain)
            .expect_err("older packages must not silently ignore terrain semantics");
        assert!(
            error
                .to_string()
                .contains("require sdkVersion 0.4.0, 0.5.0, or 0.6.0")
        );

        let current_terrain = r##"{
            "sdkVersion":"0.5.0",
            "worlds":{"maze":{"terrain":{"operations":[{
                "shape":"block","operation":"fill","position":[0,0,0],
                "size":[2,2,2],"material":"builtin:grass"
            }]}}}
        }"##;
        GamePackageDefinition::parse(current_terrain)
            .expect("current SDK should retain terrain support");
    }

    #[test]
    fn retains_authored_render_fields() {
        let package = GamePackageDefinition::parse(
            r##"{
                "startWorld":"lobby",
                "palette":{"paper":"#ffffff"},
                "world":{
                    "groundSize":120,
                    "gridSize":84,
                    "gridDivisions":21,
                    "spawn":[1,2,3],
                    "showSpawnPad":false,
                    "presentationBounds":{"minimum":[-4,-2,-3],"maximum":[5,7,6]},
                    "clouds":[{"position":[4,5,6],"scale":1.5}]
                },
                "launchPads":[{
                    "code":"GATE 01",
                    "label":"SUN COURT",
                    "position":[-10,0,-3],
                    "color":"#ed725b"
                }],
                "blocks":[{
                    "position":[0,1,2],
                    "size":[3,4,5],
                    "rotation":[0,0,0.25],
                    "color":"paper",
                    "castShadow":false,
                    "outline":false
                }]
            }"##,
        )
        .expect("package should parse");
        let worlds = package.world_entries();
        let lobby = &worlds[0].1;
        assert_eq!(lobby.world.grid_size, 84.0);
        assert_eq!(lobby.world.grid_divisions, 21);
        assert!(!lobby.world.show_spawn_pad);
        let bounds = lobby.world.presentation_bounds.unwrap();
        assert_eq!(bounds.minimum, [-4.0, -2.0, -3.0]);
        assert_eq!(bounds.maximum, [5.0, 7.0, 6.0]);
        assert_eq!(lobby.world.clouds[0].position(), [4.0, 5.0, 6.0]);
        assert_eq!(lobby.launch_pads[0].label, "SUN COURT");
        assert!(!lobby.blocks[0].outline);
        assert!(!lobby.blocks[0].cast_shadow);
        assert!((lobby.blocks[0].rotation()[2] - 0.25).abs() < 0.0001);

        let defaults = GamePackageDefinition::parse(r#"{"blocks":[{}]}"#)
            .expect("legacy blocks should retain shadow casting by default");
        assert!(defaults.world_entries()[0].1.blocks[0].cast_shadow);
    }

    #[test]
    fn retains_source_style_environment_controls() {
        let package = GamePackageDefinition::parse(
            r#"{
                "sdkVersion":"0.5.0",
                "world":{"visual":{
                    "colorCorrection":{"brightness":0.12,"contrast":0.2,"saturation":0.6},
                    "daylight":{"timeOfDay":6.5,"geographicLatitude":45,"brightness":2,"outdoorAmbient":[0.5,0.5,0.5],"shadowSoftness":0.5},
                    "sunRays":{"intensity":0.058,"spread":0.463}
                }}
            }"#,
        )
        .expect("environment controls should parse in the current SDK");
        let visual = &package.world_entries()[0].1.world.visual;
        assert_eq!(visual.color_correction.unwrap().saturation, 0.6);
        assert_eq!(visual.daylight.unwrap().time_of_day, 6.5);
        assert_eq!(visual.sun_rays.unwrap().spread, 0.463);
    }

    #[test]
    fn rejects_invalid_presentation_bounds() {
        let error = GamePackageDefinition::parse(
            r#"{"world":{"presentationBounds":{"minimum":[0,0,0],"maximum":[0,2,3]}}}"#,
        )
        .expect_err("zero-width presentation bounds must not be accepted");
        assert!(error.to_string().contains("presentationBounds"));
    }

    #[test]
    fn parses_and_validates_authored_world_camera() {
        let package = GamePackageDefinition::parse(
            r#"{"sdkVersion":"0.5.0","world":{"camera":{"yaw":1.25,"pitch":0.4,"distance":18}}}"#,
        )
        .expect("authored camera should parse");
        assert_eq!(package.world.camera.unwrap().distance, 18.0);

        for camera in [
            r#"{"yaw":null,"pitch":0,"distance":8}"#,
            r#"{"yaw":0,"pitch":2,"distance":8}"#,
            r#"{"yaw":0,"pitch":0,"distance":121}"#,
        ] {
            let source = format!(r#"{{"sdkVersion":"0.5.0","world":{{"camera":{camera}}}}}"#);
            assert!(GamePackageDefinition::parse(&source).is_err());
        }
    }

    #[test]
    fn rejects_sdk_04_for_sdk_05_world_fields() {
        for field in [
            r#""collision":{"formatVersion":1,"triangles":[[[0,0,0],[1,0,0],[0,0,1]]]}}"#,
            r#""world":{"camera":{"yaw":0,"pitch":0,"distance":8}}}"#,
            r#""world":{"physics":{"horizontalBounds":{"minimum":[-1,-1],"maximum":[1,1]}}}}"#,
            r#""world":{"visual":{"sunRays":{"intensity":0.058,"spread":0.463}}}}"#,
        ] {
            let source = format!(r#"{{"sdkVersion":"0.4.0",{field}"#);
            let error = GamePackageDefinition::parse(&source)
                .expect_err("SDK 0.4 must not silently ignore SDK 0.5 fields");
            assert!(error.to_string().contains("require sdkVersion 0.5.0"));
        }
    }

    #[test]
    fn rejects_out_of_range_environment_controls() {
        let error = GamePackageDefinition::parse(
            r#"{"sdkVersion":"0.5.0","world":{"visual":{"daylight":{"timeOfDay":24}}}}"#,
        )
        .expect_err("24:00 is outside the documented half-open day range");
        assert!(error.to_string().contains("world.visual"));
    }

    #[test]
    fn parses_general_survival_rules_without_obby_fields() {
        let package = GamePackageDefinition::parse(
            r##"{
                "startWorld":"survival",
                "lobby":false,
                "worlds":{
                    "survival":{
                        "world":{
                            "spawn":[0,1,2],
                            "health":{"max":75,"start":50},
                            "respawn":{"mode":"spawn","delay":1.2}
                        },
                        "hazards":[{
                            "id":"deep-water",
                            "kind":"damage",
                            "position":[0,0.25,0],
                            "size":[10,0.5,10],
                            "damagePerSecond":18
                        }]
                    }
                }
            }"##,
        )
        .expect("survival package should parse");
        let world = package
            .world_entries()
            .into_iter()
            .find(|(id, _)| id == "survival")
            .expect("survival world should be present")
            .1;
        assert_eq!(world.world.health.max, 75.0);
        assert_eq!(world.world.health.start, 50.0);
        assert_eq!(world.world.respawn.mode, "spawn");
        assert_eq!(world.hazards.len(), 1);
        assert_eq!(world.hazards[0].damage_per_second, 18.0);
    }

    #[test]
    fn phase5_character_member_is_additive_and_bounded() {
        let package = GamePackageDefinition::parse(
            r##"{
                "avatars": {
                    "player": {
                        "skin": "#e8ae86",
                        "shirt": "#2d6663",
                        "character": {
                            "version": 1,
                            "body": "cuba:person.v1",
                            "face": "happy",
                            "outfit": "cuba:everyday-hoodie.v1",
                            "equipment": {"hat": "cuba:star-cap.v1"},
                            "colors": {"sole": "#f6f1e7"}
                        }
                    }
                }
            }"##,
        )
        .expect("phase 5 appearance should parse");
        let character = package
            .avatars
            .player
            .as_ref()
            .and_then(|avatar| avatar.character.as_ref())
            .expect("character member");
        assert_eq!(character.body.as_deref(), Some("cuba:person.v1"));
        assert_eq!(
            character.equipment.get("hat").map(String::as_str),
            Some("cuba:star-cap.v1")
        );
        assert!(character.bounded());
    }

    #[test]
    fn old_avatar_shape_still_parses_without_character_data() {
        let package = GamePackageDefinition::parse(
            r##"{"avatars":{"player":{"skin":"#ffffff","shirt":"#000000"}}}"##,
        )
        .expect("legacy avatar should parse");
        assert!(package.avatars.player.unwrap().character.is_none());
    }

    #[test]
    fn disabled_lobby_starts_in_the_launch_destination() {
        let package = GamePackageDefinition::parse(
            r#"{
                "lobby": false,
                "startWorld": "lobby",
                "launch": {"destinationWorld": "arena"},
                "worlds": {"arena": {}}
            }"#,
        )
        .expect("package should parse");

        assert_eq!(package.initial_world_id(), Some("arena"));
        assert_eq!(package.world_entries().len(), 2);
    }

    #[test]
    fn interaction_world_contract_parses_generic_zones() {
        let package = GamePackageDefinition::parse(
            r##"{
                "worlds": {
                    "real-game": {
                        "interactions": [{
                            "id": "blue-button",
                            "kind": "zone",
                            "label": "BLUE BUTTON",
                            "position": [-4, 0, -8],
                            "radius": 3,
                            "color": "#5bd6d0"
                        }]
                    }
                }
            }"##,
        )
        .expect("interaction contract should parse");
        let interaction = package.world_entries()[1].1.interactions[0].clone();
        assert_eq!(interaction.kind, "zone");
        assert_eq!(interaction.position(), [-4.0, 0.0, -8.0]);
        assert_eq!(interaction.label, "BLUE BUTTON");
    }

    #[test]
    fn effect_templates_are_versioned_and_composed_from_generic_nodes() {
        let package = GamePackageDefinition::parse(
            r##"{
                "effects": {
                    "version": 1,
                    "templates": {
                        "finish-flash": {
                            "duration": 1.5,
                            "nodes": [{
                                "shape": "ring",
                                "size": [2, 0.1, 1],
                                "color": "#5bd6d0",
                                "variants": [
                                    {"visibleStates": ["closed"], "opacity": 0.2},
                                    {
                                        "visibleStates": ["open"],
                                        "opacity": 1,
                                        "animation": {"pulseAmount": 0.1}
                                    }
                                ],
                                "animation": {"expandAmount": 3, "fade": true}
                            }]
                        }
                    }
                },
                "worlds": {
                    "arena": {
                        "interactions": [{"id": "finish", "visual": "finish-flash"}]
                    }
                }
            }"##,
        )
        .expect("generic effect contract should parse");

        assert_eq!(package.effects.version, crate::effects::EFFECTS_VERSION);
        let template = &package.effects.templates["finish-flash"];
        assert_eq!(template.duration, 1.5);
        assert_eq!(template.nodes[0].shape, "ring");
        assert_eq!(template.nodes[0].animation.expand_amount, 3.0);
        assert_eq!(template.nodes[0].variants[0].opacity, Some(0.2));
        assert_eq!(
            template.nodes[0].variants[1].animation.pulse_amount,
            Some(0.1)
        );
        assert_eq!(
            package.world_entries()[1].1.interactions[0]
                .visual
                .as_deref(),
            Some("finish-flash")
        );
    }
}
