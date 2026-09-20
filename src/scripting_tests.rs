use super::*;

#[cfg(test)]
mod tests {
    use super::{GameScript, InteractionScriptState, InteractionZoneState};
    use crate::ui::{UiInsets, UiPointerPhase, UiRuntime, UiViewport};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn load(source: &str) -> (GameScript, Rc<RefCell<UiRuntime>>) {
        let ui = Rc::new(RefCell::new(UiRuntime::default()));
        let script = GameScript::load(source, Rc::clone(&ui)).expect("script should load");
        (script, ui)
    }

    fn load_with_worlds(
        source: &str,
        world_id: &str,
        world_ids: &[&str],
    ) -> Result<(GameScript, Rc<RefCell<UiRuntime>>), String> {
        let ui = Rc::new(RefCell::new(UiRuntime::default()));
        let script = GameScript::load_with_worlds(
            source,
            Rc::clone(&ui),
            world_id,
            world_ids.iter().map(|id| (*id).to_owned()).collect(),
        )?;
        Ok((script, ui))
    }

    #[test]
    fn configured_external_game_script_compiles() {
        let Ok(path) = std::env::var("CUBACADABRA_TEST_GAME_SCRIPT") else {
            return;
        };
        let source = std::fs::read_to_string(&path).expect("configured game script should exist");
        load(&source);
    }

    #[test]
    fn world_api_reads_current_id_and_queues_a_package_transition() {
        let source = r#"
            local game = {}
            function game.on_start(api)
                api.lobby:set_status(api.world:get_id())
                api.world:enter("maze")
            end
            return game
        "#;
        let (script, _) = load_with_worlds(source, "lobby", &["lobby", "maze"])
            .expect("world API script should load");

        assert_eq!(script.state().borrow().lobby_status, "lobby");
        assert_eq!(script.take_world_transition().as_deref(), Some("maze"));
    }

    #[test]
    fn world_api_rejects_invalid_ids_without_queueing_a_transition() {
        for requested_id in ["", "   ", "missing"] {
            let source = format!(
                r#"
                    local game = {{}}
                    function game.on_tick(api, delta)
                        api.world:enter("{requested_id}")
                    end
                    return game
                "#
            );
            let (script, _) = load_with_worlds(&source, "lobby", &["lobby", "maze"])
                .expect("invalid world request should not fail script loading");
            assert!(script.tick(1.0 / 60.0).is_err());
            assert!(script.take_world_transition().is_none());
        }
    }

    #[test]
    fn configured_maze_debug_skip_activates() {
        let Ok(path) = std::env::var("CUBACADABRA_TEST_MAZE_GAME_SCRIPT") else {
            return;
        };
        let source =
            std::fs::read_to_string(&path).expect("configured Maze 101 script should exist");
        let (script, ui) = load(&source);
        ui.borrow_mut().set_viewport(UiViewport {
            width: 390.0,
            height: 844.0,
            scale: 1.0,
            safe_area: UiInsets::default(),
        });

        let spawn = crate::types::PlayerEvent::Spawn {
            health: 100.0,
            max_health: 100.0,
            deaths: 0,
        };
        script
            .player_event(&spawn)
            .expect("initial spawn should run");
        script.player_event(&spawn).expect("maze spawn should run");

        let (x, y) = {
            let frame = ui.borrow_mut().frame().clone();
            let node = frame.nodes.iter().find(|node| node.id == "maze-debug-end");
            if !cfg!(debug_assertions) {
                assert!(node.is_none(), "release should hide the debug skip");
                return;
            }
            let node = node.expect("debug should show the skip control");
            (
                node.rect.x + node.rect.width / 2.0,
                node.rect.y + node.rect.height / 2.0,
            )
        };

        assert!(ui.borrow_mut().pointer(1, UiPointerPhase::Down, x, y));
        assert!(ui.borrow_mut().pointer(1, UiPointerPhase::Up, x, y));
        script.tick(0.0).expect("skip action should run");
        assert_eq!(
            script.state().borrow().lobby_status,
            "DEBUG: all SUNSHORE items collected. Reach the exit to continue."
        );
        assert_eq!(
            ui.borrow_mut()
                .frame()
                .nodes
                .iter()
                .find(|node| node.id == "maze-status")
                .map(|node| node.text.as_str()),
            Some("DEBUG SKIP  •  ALL COINS FOUND  •  FIND THE EXIT")
        );
    }

    #[test]
    fn loads_luau_lifecycle_callbacks() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.lobby:set_status("ready")
                end
                function game.on_tick(_api, _delta)
                end
                return game
            "#,
        );

        assert_eq!(script.state().borrow().lobby_status, "ready");
        script.tick(1.0 / 60.0).expect("tick should run");
    }

    #[test]
    fn luau_receives_the_host_build_mode() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.lobby:set_status(api.build_mode)
                end
                return game
            "#,
        );

        let expected = if cfg!(debug_assertions) {
            "DEBUG"
        } else {
            "RELEASE"
        };
        assert_eq!(script.state().borrow().lobby_status, expected);
    }

    #[test]
    fn nonterminating_startup_is_rejected_by_the_execution_budget() {
        let result = GameScript::load(
            r#"
                local game = {}
                function game.on_start(_api)
                    while true do end
                end
                return game
            "#,
            Rc::new(RefCell::new(UiRuntime::default())),
        );

        let error = match result {
            Ok(_) => panic!("nonterminating startup should be rejected"),
            Err(error) => error,
        };
        assert!(error.contains("execution budget exceeded"));
    }

    #[test]
    fn nonterminating_tick_is_rejected_by_the_execution_budget() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_tick(_api, _delta)
                    while true do end
                end
                return game
            "#,
        );

        let error = script.tick(0.0).unwrap_err();
        assert!(error.contains("execution budget exceeded"));
    }

    #[test]
    fn task_scheduler_orders_spawn_defer_delay_and_wait_on_simulation_time() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    task.spawn(function()
                        api.lobby:set_status("spawn")
                        local elapsed = task.wait(0.2)
                        if elapsed >= 0.2 then
                            api.lobby:set_status("wait")
                        end
                    end)
                    task.defer(function()
                        api.lobby:set_status("defer")
                    end)
                    task.delay(0.3, function()
                        api.lobby:set_status("delay")
                    end)
                end
                return game
            "#,
        );

        script.tick(0.1).unwrap();
        assert_eq!(script.state().borrow().lobby_status, "defer");
        script.tick(0.1).unwrap();
        assert_eq!(script.state().borrow().lobby_status, "defer");
        script.tick(0.1).unwrap();
        assert_eq!(script.state().borrow().lobby_status, "delay");
    }

    #[test]
    fn failed_task_does_not_stop_other_tasks() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    task.spawn(function()
                        error("task boom")
                    end)
                    task.spawn(function()
                        api.lobby:set_status("healthy")
                    end)
                end
                return game
            "#,
        );

        script.tick(0.0).unwrap();
        assert_eq!(script.state().borrow().lobby_status, "healthy");
        assert!(
            script
                .state()
                .borrow()
                .last_error
                .as_deref()
                .unwrap()
                .contains("task boom")
        );
    }

    #[test]
    fn task_waits_are_cancellable_and_tight_loops_are_budgeted() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    local delayed = task.delay(1, function()
                        api.lobby:set_status("cancelled task ran")
                    end)
                    task.cancel(delayed)
                    task.spawn(function()
                        while true do end
                    end)
                    task.defer(function()
                        api.lobby:set_status("healthy after budget")
                    end)
                end
                return game
            "#,
        );

        script.tick(0.0).unwrap();
        assert_eq!(script.state().borrow().lobby_status, "");
        script.tick(0.0).unwrap();
        assert_eq!(script.state().borrow().lobby_status, "healthy after budget");
    }

    #[test]
    fn explicit_save_and_restore_hooks_round_trip_json_state() {
        let (script, _) = load(
            r#"
                local game = { score = 7 }
                function game.on_save(_api)
                    return { score = game.score, nested = { ready = true } }
                end
                function game.on_restore(_api, state)
                    game.score = state.score
                end
                return game
            "#,
        );

        let state = script
            .save_state()
            .expect("save hook should return JSON state");
        assert_eq!(state["score"], 7);
        script
            .restore_state(&serde_json::json!({ "score": 42 }))
            .expect("restore hook should accept JSON state");
        assert_eq!(script.save_state().unwrap()["score"], 42);
    }

    #[test]
    fn luau_receives_obby_player_events() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_player_event(api, event)
                    api.lobby:set_status(event.type .. ":" .. event.kind .. ":" .. event.deaths)
                end
                return game
            "#,
        );

        script
            .player_event(&crate::types::PlayerEvent::Death {
                cause: "fall".to_owned(),
                checkpoint: "tower".to_owned(),
                deaths: 3,
                health: 0.0,
                max_health: 100.0,
            })
            .expect("player event callback should run");
        assert_eq!(script.state().borrow().lobby_status, "player:death:3");
    }

    #[test]
    fn luau_can_disable_the_lobby() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.lobby:set_enabled(false)
                end
                return game
            "#,
        );

        assert_eq!(script.lobby_enabled_override(), Some(false));
    }

    #[test]
    fn luau_can_read_generic_interactions_and_receive_events() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_interaction(api, event)
                    api.lobby:set_status(event.id .. ":" .. event.phase .. ":" .. event.position[1])
                end
                function game.on_tick(api)
                    local state = api.interactions:get_state()
                    if state.zones.button.inside then
                        api.lobby:set_status("inside:" .. state.zones.button.players)
                    end
                end
                return game
            "#,
        );

        script.set_interaction_state(InteractionScriptState {
            zones: vec![InteractionZoneState {
                id: "button".to_owned(),
                kind: "zone".to_owned(),
                label: "BUTTON".to_owned(),
                inside: true,
                nearby: true,
                players: 2,
            }],
            event_id: 1,
        });
        script
            .interaction(&crate::types::InteractionEvent {
                id: "button".to_owned(),
                phase: "enter".to_owned(),
                players: 2,
                position: [4.0, 0.5, -2.0],
            })
            .expect("interaction callback should run");
        assert_eq!(script.state().borrow().lobby_status, "button:enter:4");
        script.tick(0.0).expect("tick should run");
        assert_eq!(script.state().borrow().lobby_status, "inside:2");
    }

    #[test]
    fn luau_can_read_the_shared_interaction_zone_schema() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    local schema = api.interactions:get_schema("cuba:interaction-zone.v1")
                    local label_name = "missing"
                    local radius_name = "missing"
                    for _, property in ipairs(schema.properties) do
                        if property.id == "cuba:interaction-zone.label" then
                            label_name = property.name
                        elseif property.id == "cuba:interaction-zone.radius" then
                            radius_name = property.name
                        end
                    end
                    api.lobby:set_status(schema.name .. ":" .. label_name .. "/" .. radius_name)
                end
                return game
            "#,
        );

        assert_eq!(
            script.state().borrow().lobby_status,
            "InteractionZone:Label/Radius"
        );
    }

    #[test]
    fn luau_can_publish_and_receive_opaque_network_messages() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.network:set_state("progress", { learned = 2 })
                end
                function game.on_network_message(api, message)
                    if message.type == "game_state" and message.channel == "progress" then
                        api.lobby:set_status("learned:" .. message.payload.learned)
                    end
                end
                return game
            "#,
        );

        script
            .take_network_message()
            .expect("on_start should emit a retained state message");
        assert!(script.enqueue_network_message(
            r#"{"type":"game_state","channel":"progress","payload":{"learned":3}}"#,
        ));
        script.tick(0.0).expect("network callback should run");
        assert_eq!(script.state().borrow().lobby_status, "learned:3");
    }

    #[test]
    fn luau_can_compare_and_set_authoritative_state() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.network:compare_set_state("round", 7, {
                        round = 3,
                        checkpoints = { gate_a = true },
                    })
                end
                return game
            "#,
        );

        let message = script
            .take_network_message()
            .expect("compare_set_state should emit a network command");
        let value: serde_json::Value = serde_json::from_str(&message).unwrap();
        assert_eq!(value["channel"], "round");
        assert_eq!(value["expectedSequence"], 7);
        assert_eq!(value["payload"]["round"], 3);
        assert_eq!(value["payload"]["checkpoints"]["gate_a"], true);
    }

    #[test]
    fn luau_can_control_manifest_defined_effects() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.effects:set_state("gate-a", "open")
                    api.effects:play("finish-flash", { position = { 1, 2, 3 } })
                end
                return game
            "#,
        );

        assert_eq!(
            script.take_effect_commands(),
            vec![
                crate::effects::EffectCommand::SetState {
                    target: "gate-a".to_owned(),
                    state: "open".to_owned(),
                },
                crate::effects::EffectCommand::Play {
                    template: "finish-flash".to_owned(),
                    position: [1.0, 2.0, 3.0],
                },
            ]
        );
    }

    #[test]
    fn script_queues_are_bounded() {
        let (script, _) = load("return {}");
        let message = format!(r#"{{"payload":"{}"}}"#, "x".repeat(4096));
        let mut accepted = 0;
        while script.enqueue_network_message(&message) {
            accepted += 1;
        }
        assert!(accepted > 0);
        assert!(accepted < 64);
    }

    #[test]
    fn luau_can_emit_bounded_game_audio_commands() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    api.audio:play("success-chime", { volume = 0.75 })
                end
                return game
            "#,
        );

        let message = script
            .take_audio_message()
            .expect("on_start should emit an audio command");
        let value: serde_json::Value = serde_json::from_str(&message).unwrap();
        assert_eq!(value["type"], "play");
        assert_eq!(value["id"], "success-chime");
        assert_eq!(value["volume"], 0.75);
    }

    #[test]
    fn audio_overflow_keeps_the_newest_commands_without_stopping_the_game() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_start(api)
                    for index = 1, 80 do
                        api.audio:play("sound-" .. index)
                    end
                    api.lobby:set_status("still running")
                end
                return game
            "#,
        );

        assert_eq!(script.state().borrow().lobby_status, "still running");
        let mut messages = Vec::new();
        while let Some(message) = script.take_audio_message() {
            messages.push(serde_json::from_str::<serde_json::Value>(&message).unwrap());
        }
        assert_eq!(messages.len(), 64);
        assert_eq!(messages.first().unwrap()["id"], "sound-17");
        assert_eq!(messages.last().unwrap()["id"], "sound-80");
    }

    #[test]
    fn launch_callback_receives_pad_and_players() {
        let (script, _) = load(
            r#"
                local game = {}
                function game.on_launch(api, launch)
                    api.session:start(launch.pad_id .. ":" .. #launch.player_ids, {})
                end
                return game
            "#,
        );

        script.launch("pad-2", &[4, 9]).expect("launch should run");
        assert_eq!(
            script.state().borrow().session_name.as_deref(),
            Some("pad-2:2")
        );
    }

    #[test]
    fn luau_can_declare_ui_and_receive_actions() {
        let (script, ui) = load(
            r##"
                local game = {}
                function game.on_start(api)
                    api.ui:set_document([[{
                        "nodes":[{
                            "id":"settings",
                            "kind":"button",
                            "text":"SETTINGS",
                            "action":"open-settings",
                            "layout":{"width":120,"height":48}
                        }]
                    }]])
                end
                function game.on_ui_event(api, event)
                    if event.action == "open-settings" then
                        api.lobby:set_status("settings-open")
                    end
                end
                return game
            "##,
        );
        ui.borrow_mut().set_viewport(UiViewport {
            width: 390.0,
            height: 844.0,
            scale: 1.0,
            safe_area: UiInsets::default(),
        });
        assert!(ui.borrow_mut().pointer(1, UiPointerPhase::Down, 10.0, 10.0));
        assert!(ui.borrow_mut().pointer(1, UiPointerPhase::Up, 10.0, 10.0));
        script.tick(0.0).unwrap();
        assert_eq!(script.state().borrow().lobby_status, "settings-open");
    }

    #[test]
    fn luau_can_declare_ui_with_tables() {
        let (_, ui) = load(
            r##"
                local game = {}
                function game.on_start(api)
                    api.ui:set_document({
                        nodes = {
                            {
                                id = "dock",
                                kind = "panel",
                                layout = {
                                    anchor = "bottom",
                                    width = "90%",
                                    maxWidth = 520,
                                    height = 60,
                                },
                                children = {
                                    { id = "place", kind = "button", text = "PLACE" },
                                },
                            },
                        },
                    })
                end
                return game
            "##,
        );
        ui.borrow_mut().set_viewport(UiViewport {
            width: 390.0,
            height: 844.0,
            scale: 1.0,
            safe_area: UiInsets::default(),
        });
        let frame = ui.borrow_mut().frame().clone();
        assert_eq!(frame.nodes[0].id, "dock");
        assert_eq!(frame.nodes[1].id, "place");
        assert_eq!(frame.nodes[0].rect.width, 351.0);
    }
}
