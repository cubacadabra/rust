    use super::*;

    fn runtime(source: &str, width: f32, height: f32) -> UiRuntime {
        let mut runtime = UiRuntime::default();
        runtime.set_viewport(UiViewport {
            width,
            height,
            scale: 1.0,
            safe_area: UiInsets {
                top: 47.0,
                right: 0.0,
                bottom: 34.0,
                left: 0.0,
            },
        });
        runtime.set_document_json(source).unwrap();
        runtime
    }

    #[test]
    fn anchors_to_safe_area_and_respects_max_width() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"header","kind":"panel","layout":{"anchor":"top","width":"fill","height":64,"maxWidth":720}}]}"##,
            1280.0,
            800.0,
        );
        let header = &runtime.frame().nodes[0];
        assert_eq!(
            header.rect,
            UiRect {
                x: 280.0,
                y: 47.0,
                width: 720.0,
                height: 64.0
            }
        );
    }

    #[test]
    fn shared_header_is_engine_owned_and_emits_no_event() {
        let mut runtime = runtime(r##"{"nodes":[]}"##, 390.0, 844.0);
        let frame = runtime.frame().clone();
        let shared_ids = frame
            .nodes
            .iter()
            .filter(|node| node.id.starts_with("__shared_header_"))
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(shared_ids.len(), 6);
        assert!(runtime.pointer(1, UiPointerPhase::Down, 32.0, 80.0));
        assert!(runtime.pointer(1, UiPointerPhase::Up, 32.0, 80.0));
        assert!(!runtime.poll_event());
    }

    #[test]
    fn shared_logo_opens_and_closes_placeholder_modal() {
        let mut runtime = runtime(r##"{"nodes":[]}"##, 390.0, 844.0);
        let logo = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.id == "__shared_header_logo_surface")
            .expect("shared logo should render")
            .rect;
        let logo_x = logo.x + logo.width * 0.5;
        let logo_y = logo.y + logo.height * 0.5;

        assert!(runtime.pointer(1, UiPointerPhase::Down, logo_x, logo_y));
        assert!(runtime.pointer(1, UiPointerPhase::Up, logo_x, logo_y));
        assert!(runtime.shared_modal_visible());
        runtime.advance(1.0);
        let frame = runtime.frame().clone();
        for label in ["About", "Settings", "People", "Report", "Help"] {
            assert!(frame.nodes.iter().any(|node| node.text == label));
        }
        assert!(frame.nodes.iter().any(|node| node.id == "__shared_modal_scrim"));
        assert!(frame.nodes.iter().any(|node| node.id == "__shared_modal_body"));
        let about_link = frame
            .nodes
            .iter()
            .find(|node| node.id == "__shared_modal_about_link")
            .expect("About tab should expose its first link option")
            .rect;
        let about_x = about_link.x + about_link.width * 0.5;
        let about_y = about_link.y + about_link.height * 0.5;
        assert!(runtime.is_external_link_at(about_x, about_y));
        assert!(runtime.pointer(2, UiPointerPhase::Down, about_x, about_y));
        assert!(runtime.pointer(2, UiPointerPhase::Up, about_x, about_y));
        assert!(runtime.poll_event());
        let event: serde_json::Value = serde_json::from_slice(runtime.event_buffer()).unwrap();
        assert_eq!(event["action"], "shared.about.open");

        // The scrim is above the header, so tapping the logo while open is
        // the same dismiss gesture as tapping anywhere outside the panel.
        assert!(runtime.pointer(3, UiPointerPhase::Down, logo_x, logo_y));
        assert!(runtime.pointer(3, UiPointerPhase::Up, logo_x, logo_y));
        runtime.advance(1.0);
        assert!(!runtime.shared_modal_visible());
        assert!(runtime
            .frame()
            .nodes
            .iter()
            .all(|node| !node.id.starts_with("__shared_modal_")));
    }

    #[test]
    fn shared_modal_tabs_are_selectable_without_script_events() {
        let mut runtime = runtime(r##"{"nodes":[]}"##, 1024.0, 768.0);
        let logo = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.id == "__shared_header_logo_surface")
            .expect("shared logo should render")
            .rect;
        let logo_x = logo.x + logo.width * 0.5;
        let logo_y = logo.y + logo.height * 0.5;
        assert!(runtime.pointer(1, UiPointerPhase::Down, logo_x, logo_y));
        assert!(runtime.pointer(1, UiPointerPhase::Up, logo_x, logo_y));
        runtime.advance(1.0);

        let people = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.text == "People")
            .expect("People placeholder tab should render")
            .rect;
        let people_x = people.x + people.width * 0.5;
        let people_y = people.y + people.height * 0.5;
        assert!(runtime.pointer(3, UiPointerPhase::Down, people_x, people_y));
        assert!(runtime.pointer(3, UiPointerPhase::Up, people_x, people_y));
        assert!(!runtime.poll_event(), "placeholder tabs are engine-owned");

        let people = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.text == "People")
            .unwrap();
        assert_eq!(people.background, Some([0.24, 0.30, 0.34, 0.98]));
    }

    #[test]
    fn game_header_region_follows_shared_header_and_bottom_region_is_compact() {
        let mut runtime = runtime(
            r##"{
                "nodes":[
                    {"id":"build","kind":"button","icon":"build","action":"build.menu","layout":{"region":"header","width":56,"height":56}},
                    {"id":"context","kind":"panel","layout":{"region":"bottomCenter","width":"auto","height":56,"padding":4,"direction":"row","gap":6},"children":[
                        {"id":"place","kind":"button","icon":"plus","action":"build.place","layout":{"width":48,"height":48}},
                        {"id":"rotate","kind":"button","icon":"rotate","action":"build.rotate","layout":{"width":48,"height":48}},
                        {"id":"remove","kind":"button","icon":"trash","action":"build.remove","layout":{"width":48,"height":48}}
                    ]}
                ]
            }"##,
            390.0,
            844.0,
        );
        let frame = runtime.frame().clone();
        let shared_controls = frame
            .nodes
            .iter()
            .find(|node| node.id == "__shared_header_controls_surface")
            .expect("shared controls should render");
        let build = frame
            .nodes
            .iter()
            .find(|node| node.id == "build")
            .expect("game header control should render");
        let context = frame
            .nodes
            .iter()
            .find(|node| node.id == "context")
            .expect("bottom context should render");
        assert!(build.rect.x >= shared_controls.rect.x + shared_controls.rect.width);
        assert_eq!(build.icon.as_deref(), Some("build"));
        assert_eq!(context.rect.width, 164.0);
        assert!(context.rect.x > 100.0 && context.rect.x + context.rect.width < 290.0);
        assert!(context.rect.y + context.rect.height <= 844.0 - 34.0);
    }

    #[test]
    fn visible_in_scopes_rendering_and_hit_testing_to_worlds() {
        let mut runtime = runtime(
            r##"{
                "nodes":[
                    {"id":"game-only","kind":"button","text":"BUILD","action":"build.place","visibleIn":["real-game"],"layout":{"width":120,"height":48,"offset":[0,140]}},
                    {"id":"game-panel","kind":"panel","visibleIn":["real-game"],"layout":{"width":160,"height":64,"offset":[140,140]},"children":[
                        {"id":"game-child","kind":"text","text":"GAME"}
                    ]}
                ]
            }"##,
            390.0,
            844.0,
        );

        assert!(
            runtime
                .frame()
                .nodes
                .iter()
                .all(|node| !matches!(node.id.as_str(), "game-only" | "game-panel" | "game-child"))
        );
        assert!(!runtime.pointer(1, UiPointerPhase::Down, 40.0, 210.0));

        runtime.set_world_id("real-game");
        let frame = runtime.frame().clone();
        assert!(frame.nodes.iter().any(|node| node.id == "game-only"));
        assert!(frame.nodes.iter().any(|node| node.id == "game-child"));
        assert!(runtime.pointer(1, UiPointerPhase::Down, 40.0, 210.0));
        assert!(runtime.pointer(1, UiPointerPhase::Up, 40.0, 210.0));
        assert!(runtime.poll_event());
        let event: serde_json::Value = serde_json::from_slice(runtime.event_buffer()).unwrap();
        assert_eq!(event["action"], "build.place");
    }

    #[test]
    fn gameplay_controls_are_persistent_across_worlds() {
        let mut runtime = runtime(
            r##"{
                "nodes":[
                    {"id":"player-joystick","kind":"joystick","action":"player.move","visible":false,"visibleIn":["real-game"],"layout":{"anchor":"bottomLeft","width":120,"height":120,"offset":[20,-24]},"style":{"background":"#091A22C9","foreground":"#EDF0E5FF"}},
                    {"id":"player-jump","kind":"button","text":"JUMP","action":"player.jump","visible":false,"visibleIn":["real-game"],"layout":{"anchor":"bottomRight","width":86,"height":44,"offset":[-22,-84]},"style":{"background":"#102D3AE8","foreground":"#F7F8EEFF"}},
                    {"id":"player-run","kind":"button","text":"RUN","action":"player.run","visible":false,"visibleIn":["real-game"],"layout":{"anchor":"bottomRight","width":86,"height":44,"offset":[-22,-30]},"style":{"background":"#102D3AE8","foreground":"#F7F8EEFF"}}
                ]
            }"##,
            1024.0,
            768.0,
        );
        let frame = runtime.frame().clone();
        for id in ["player-joystick", "player-jump", "player-run"] {
            assert!(
                frame.nodes.iter().any(|node| node.id == id),
                "persistent control {id} should render in the lobby"
            );
        }
        let joystick = frame
            .nodes
            .iter()
            .find(|node| node.id == "player-joystick")
            .expect("joystick should render");
        assert!(joystick.rect.x >= 20.0);
        assert!(joystick.rect.y + joystick.rect.height <= 768.0 - 34.0);

        let jump = frame
            .nodes
            .iter()
            .find(|node| node.id == "player-jump")
            .expect("jump should render");
        let jump_x = jump.rect.x + jump.rect.width * 0.5;
        let jump_y = jump.rect.y + jump.rect.height * 0.5;
        assert!(runtime.pointer(7, UiPointerPhase::Down, jump_x, jump_y));
        assert!(runtime.pointer(7, UiPointerPhase::Up, jump_x, jump_y));
        assert!(runtime.poll_event());
        assert!(std::str::from_utf8(runtime.event_buffer())
            .expect("event should be UTF-8")
            .contains("player.jump"));
    }

    #[test]
    fn portrait_ipad_uses_compact_shared_header_geometry() {
        let mut runtime = runtime(r##"{"nodes":[]}"##, 768.0, 1024.0);
        let frame = runtime.frame().clone();
        let logo = frame
            .nodes
            .iter()
            .find(|node| node.id == "__shared_header_logo_surface")
            .expect("portrait iPad should render the shared header");
        let controls = frame
            .nodes
            .iter()
            .find(|node| node.id == "__shared_header_controls_surface")
            .expect("portrait iPad should render shared controls");
        assert_eq!(logo.rect.width, 44.0);
        assert!(controls.rect.x + controls.rect.width < 768.0);
    }

    #[test]
    fn menu_and_modal_nodes_are_valid_container_primitives() {
        let mut runtime = runtime(
            r##"{"nodes":[
                {"id":"scrim","kind":"modal","blocksInput":true,"layout":{"width":"fill","height":"fill","ignoreSafeArea":true}},
                {"id":"menu","kind":"menu","layout":{"width":220,"height":180},"children":[
                    {"id":"cube","kind":"button","icon":"cube","text":"CUBE","action":"shape.cube","layout":{"width":96,"height":56}}
                ]}
            ]}"##,
            390.0,
            844.0,
        );
        let frame = runtime.frame().clone();
        assert_eq!(
            frame
                .nodes
                .iter()
                .find(|node| node.id == "scrim")
                .expect("scrim should render")
                .kind,
            UiNodeKind::Modal
        );
        assert_eq!(
            frame
                .nodes
                .iter()
                .find(|node| node.id == "menu")
                .expect("menu should render")
                .kind,
            UiNodeKind::Menu
        );
        assert_eq!(
            frame
                .nodes
                .iter()
                .find(|node| node.id == "cube")
                .expect("menu item should render")
                .icon
                .as_deref(),
            Some("cube")
        );
    }

    #[test]
    fn bottom_dock_stays_above_home_indicator() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"dock","kind":"panel","layout":{"anchor":"bottom","width":320,"height":56,"offset":[0,-16]}}]}"##,
            390.0,
            844.0,
        );
        let dock = &runtime.frame().nodes[0];
        assert_eq!(dock.rect.y, 738.0);
        assert!(dock.rect.y + dock.rect.height <= 844.0 - 34.0);
    }

    #[test]
    fn button_requires_release_inside_and_emits_host_event() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"build","kind":"button","text":"BUILD","action":"build.use","layout":{"width":120,"height":48,"offset":[0,140]}}]}"##,
            390.0,
            844.0,
        );
        assert!(runtime.pointer(7, UiPointerPhase::Down, 30.0, 200.0));
        assert!(runtime.pointer(7, UiPointerPhase::Up, 30.0, 200.0));
        assert!(runtime.poll_event());
        let event: serde_json::Value = serde_json::from_slice(runtime.event_buffer()).unwrap();
        assert_eq!(event["action"], "build.use");
        assert_eq!(event["phase"], "activate");
    }

    #[test]
    fn slider_updates_value_during_drag() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"music","kind":"slider","action":"settings.music","value":0.5,"layout":{"width":200,"height":44,"offset":[0,140]}}]}"##,
            390.0,
            844.0,
        );
        assert!(runtime.pointer(2, UiPointerPhase::Down, 100.0, 200.0));
        assert!(runtime.pointer(2, UiPointerPhase::Move, 180.0, 200.0));
        let slider = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.id == "music")
            .unwrap();
        assert!((slider.value - 0.9).abs() < 0.001);
    }

    #[test]
    fn rejects_duplicate_ids() {
        let mut runtime = UiRuntime::default();
        let error = runtime
            .set_document_json(r##"{"nodes":[{"id":"same"},{"id":"same"}]}"##)
            .unwrap_err();
        assert!(error.contains("Duplicate"));
    }

    #[test]
    fn modal_scrim_can_cover_unsafe_area_and_block_world_input() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"scrim","kind":"panel","blocksInput":true,"layout":{"width":"fill","height":"fill","ignoreSafeArea":true}}]}"##,
            390.0,
            844.0,
        );
        let scrim = &runtime.frame().nodes[0];
        assert_eq!(
            scrim.rect,
            UiRect {
                x: 0.0,
                y: 0.0,
                width: 390.0,
                height: 844.0
            }
        );
        assert!(runtime.pointer(11, UiPointerPhase::Down, 5.0, 5.0));
        assert!(runtime.pointer(11, UiPointerPhase::Up, 5.0, 5.0));
        assert!(!runtime.poll_event());
    }

    #[test]
    fn modal_overlay_takes_priority_over_shared_and_game_controls() {
        let mut runtime = runtime(
            r##"{"nodes":[
                {"id":"underlay","kind":"button","text":"UNDER","action":"underlay","layout":{"width":120,"height":48,"offset":[0,140]}},
                {"id":"scrim","kind":"modal","action":"close","blocksInput":true,"layout":{"width":"fill","height":"fill","ignoreSafeArea":true}}
            ]}"##,
            390.0,
            844.0,
        );
        assert!(runtime.pointer(9, UiPointerPhase::Down, 30.0, 200.0));
        assert!(runtime.pointer(9, UiPointerPhase::Up, 30.0, 200.0));
        assert!(runtime.poll_event());
        let event = std::str::from_utf8(runtime.event_buffer()).expect("event should be UTF-8");
        assert!(event.contains("\"action\":\"close\""));
    }

    #[test]
    fn oversized_popover_is_clamped_inside_the_safe_viewport() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"popover","kind":"menu","layout":{"width":336,"height":136,"offset":[380,88]}}]}"##,
            390.0,
            844.0,
        );
        let popover = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.id == "popover")
            .unwrap();
        assert!(popover.rect.x >= 0.0);
        assert!(popover.rect.x + popover.rect.width <= 390.0);
        assert!(popover.rect.y >= 47.0);
        assert!(popover.rect.y + popover.rect.height <= 810.0);
    }

    #[test]
    fn joystick_clamps_vector_and_resets_on_release() {
        let mut runtime = runtime(
            r##"{"nodes":[{"id":"move","kind":"joystick","action":"player.move","layout":{"width":120,"height":120,"offset":[0,140]}}]}"##,
            390.0,
            844.0,
        );
        assert!(runtime.pointer(4, UiPointerPhase::Down, 60.0, 247.0));
        assert!(runtime.pointer(4, UiPointerPhase::Move, 180.0, 247.0));
        let stick = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.id == "move")
            .unwrap();
        assert_eq!(stick.value_x, 1.0);
        assert_eq!(stick.value_y, 0.0);
        assert!(runtime.pointer(4, UiPointerPhase::Up, 180.0, 247.0));
        let stick = runtime
            .frame()
            .nodes
            .iter()
            .find(|node| node.id == "move")
            .unwrap();
        assert_eq!((stick.value_x, stick.value_y), (0.0, 0.0));
        assert!(runtime.host_events.iter().any(|event| {
            event.phase == "release" && event.x == Some(0.0) && event.y == Some(0.0)
        }));
    }
