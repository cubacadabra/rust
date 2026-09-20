#[test]
fn legacy_remote_slot_generation_changes_at_replacement_boundary() {
    let mut engine = Engine::new();
    engine.set_remote_player_count(1);
    let first = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("remote sample")
        .key
        .generation;
    engine.set_remote_player_count(0);
    engine.set_remote_player_count(1);
    let replacement = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("replacement sample")
        .key
        .generation;
    assert_ne!(first, replacement);
}

#[test]
fn local_npcs_are_disabled_until_authoritative() {
    let mut engine = Engine::new();
    for _ in 0..181 {
        engine.step(1.0 / 60.0);
    }
    assert!(engine.agents.is_empty());
    assert_eq!(engine.agent_count(), 0);
}

#[test]
fn remote_players_are_written_to_the_snapshot() {
    let mut engine = Engine::new();
    engine.set_remote_player_count(1);
    engine.set_remote_player(0, [4.0, 0.0, -6.0], 0.75, true, false);
    engine.step(1.0 / 60.0);

    assert_eq!(engine.remote_player_count(), 1);
    assert_eq!(engine.agent_count(), 1);
    assert_eq!(
        &engine.snapshot[SNAPSHOT_STRIDE..SNAPSHOT_STRIDE + 3],
        &[4.0, 0.0, -6.0]
    );
    assert_eq!(engine.snapshot[SNAPSHOT_STRIDE + 3], 0.75);
    assert!(engine.snapshot[SNAPSHOT_STRIDE + 4] > 0.0);
    assert_eq!(engine.snapshot[SNAPSHOT_STRIDE + 6], -1.0);
}

#[test]
fn spawned_remote_players_get_distinct_vibrant_hoodies() {
    let mut engine = Engine::new();
    engine.set_remote_player_count(MAX_AGENTS);

    let colors: Vec<_> = engine
        .remote_players
        .iter()
        .map(|player| player.appearance.colors.primary)
        .collect();
    assert!(colors.iter().all(|color| *color != [0.18, 0.40, 0.39, 1.0]));
    for (index, color) in colors.iter().enumerate() {
        assert!(colors[index + 1..].iter().all(|other| other != color));
    }
}

#[test]
fn snapshot_abi_fixture_preserves_entity_suffix_meanings() {
    let mut engine = Engine::new();
    engine.view_yaw = 1.25;
    engine.player.position = [2.0, 0.0, -3.0];
    engine.player.walk_cycle = 0.75;
    engine.player.grounded = true;
    engine.player.moving = true;
    engine.player.sprinting = true;
    engine.write_snapshot();

    assert_eq!(engine.snapshot.len(), 18 * SNAPSHOT_STRIDE);
    assert_eq!(
        &engine.snapshot[..SNAPSHOT_STRIDE],
        &[2.0, 0.0, -3.0, 1.25, 0.75, 1.0, 1.0, 1.0]
    );

    engine.set_remote_player_count(1);
    engine.set_remote_player(0, [4.0, 0.0, -6.0], -0.5, true, true);
    engine.write_snapshot();
    assert_eq!(
        &engine.snapshot[SNAPSHOT_STRIDE..2 * SNAPSHOT_STRIDE],
        &[4.0, 0.0, -6.0, -0.5, 0.0, 1.0, -1.0, 0.0]
    );

    let mut npc_engine = Engine::new();
    npc_engine.agents.push(Agent {
        position: [1.0, 0.0, 2.0],
        target: crate::math::Vec2 { x: 1.0, z: 2.0 },
        meeting_target: crate::math::Vec2 { x: 1.0, z: 2.0 },
        meeting_index: 3,
        phase: AgentPhase::Assembling,
        spawned_at: 0.0,
        next_decision_at: 0.0,
        gather_at: 0.0,
        next_jump_at: 0.0,
        speed: 1.0,
        walk_cycle: 0.0,
        vertical_velocity: 0.0,
        grounded: true,
    });
    npc_engine.write_snapshot();
    assert_eq!(
        &npc_engine.snapshot[SNAPSHOT_STRIDE..2 * SNAPSHOT_STRIDE],
        &[1.0, 0.0, 2.0, 0.0, 0.0, 2.0, 3.0, 0.0]
    );

    engine.set_remote_player_count(50);
    assert_eq!(engine.remote_player_count(), 17);
    assert_eq!(engine.snapshot.len(), 18 * SNAPSHOT_STRIDE);
}

#[test]
fn typed_motion_keeps_local_sprint_separate_from_npc_gathering() {
    let mut engine = Engine::new();
    engine.set_input(Input {
        forward: 1.0,
        sprint: true,
        ..Input::default()
    });
    engine.step(1.0 / 60.0);
    let samples: Vec<_> = engine.character_motion_samples().collect();
    let local = samples
        .iter()
        .find(|sample| sample.key.kind == CharacterEntityKind::LocalPlayer)
        .expect("local sample");
    assert!(local.moving);
    assert!(local.sprinting);
    assert_eq!(local.support, CharacterSupport::Grounded { height: 0.0 });
    assert_eq!(engine.snapshot[7], 1.0);

    engine.set_remote_player_count(1);
    let remote = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("remote sample");
    assert_eq!(remote.support, CharacterSupport::Unknown);
    assert!(remote.planar_velocity.is_none());
    assert!(remote.vertical_velocity.is_none());

    engine.agents.push(Agent {
        position: [1.0, 0.0, 2.0],
        target: crate::math::Vec2 { x: 1.0, z: 2.0 },
        meeting_target: crate::math::Vec2 { x: 1.0, z: 2.0 },
        meeting_index: 0,
        phase: AgentPhase::Assembling,
        spawned_at: 0.0,
        next_decision_at: 0.0,
        gather_at: 0.0,
        next_jump_at: 0.0,
        speed: 1.0,
        walk_cycle: 0.0,
        vertical_velocity: 0.0,
        grounded: true,
    });
    let npc = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::LocalNpc)
        .expect("NPC sample");
    assert!(npc.moving);
    assert!(!npc.sprinting);
    assert_eq!(npc.support, CharacterSupport::Grounded { height: 0.0 });
}

#[test]
fn render_only_capacity_does_not_change_engine_capacity() {
    let mut engine = Engine::new();
    engine.set_remote_player_count(17);
    assert_eq!(engine.remote_player_count(), 17);
    assert_eq!(engine.agent_count(), 17);
    assert_eq!(engine.snapshot().len(), 18 * SNAPSHOT_STRIDE);
}

#[test]
fn local_appearance_is_atomic_and_revisioned() {
    let mut engine = Engine::new();
    assert_eq!(
        engine.set_local_appearance_json(
            r##"{
                "version":1,
                "body":"cuba:cat.v1",
                "face":"curious",
                "outfit":"cuba:everyday-hoodie.v1",
                "colors":{"primary":"#176b87"},
                "revision":2
            }"##,
        ),
        1
    );
    assert_eq!(engine.player_appearance.body, crate::character::BodyId::Cat);
    assert_eq!(engine.player_appearance.revision, 2);
    assert!((engine.player_appearance.colors.primary[0] - 23.0 / 255.0).abs() < 1e-6);

    assert_eq!(
        engine
            .set_local_appearance_json(r##"{"version":1,"body":"cuba:dragon.v1","revision":1}"##,),
        2
    );
    assert_eq!(engine.player_appearance.body, crate::character::BodyId::Cat);
}

#[test]
fn local_appearance_can_switch_between_authored_person_bases_repeatedly() {
    let mut engine = Engine::new();
    for (revision, base) in [
        (1, "cuba:base/person-02.v1"),
        (2, "cuba:base/person.v1"),
        (3, "cuba:base/person-02.v1"),
    ] {
        let appearance = format!(
            r#"{{"version":1,"body":"cuba:person.v1","equipment":{{"base":"{base}"}},"revision":{revision}}}"#
        );
        assert_eq!(engine.set_local_appearance_json(&appearance), 1);
        let selected = engine
            .player_appearance
            .equipment
            .iter()
            .find(|item| item.slot == EquipmentSlot::Base)
            .unwrap();
        assert_eq!(selected.asset_id, base);
    }
}

#[test]
fn v2_morph_loadouts_remain_native_in_the_renderer_contract() {
    let mut engine = Engine::new();
    assert_eq!(
        engine.set_local_morph_loadout_json(
            r##"{"version":2,"base":"cuba:base/person-02.v1","parts":["cuba:hair/buzz.v1","cuba:top/person-top.v1","cuba:bottom/person-bottom.v1","cuba:footwear/person-shoes.v1"],"face":"cuba:face/surprised.v1","parameters":{},"revision":1}"##,
        ),
        1
    );
    assert_eq!(
        engine.player_appearance.face,
        crate::character::FacePreset::Surprised
    );
    assert!(engine.player_appearance.equipment.is_empty());
    let loadout = engine.player_appearance.morph_loadout.as_ref().unwrap();
    assert_eq!(loadout.base.as_str(), "cuba:base/person-02.v1");
    assert_eq!(loadout.parts.len(), 4);
}

#[test]
fn versioned_remote_roster_preserves_identity_through_reorder() {
    let mut engine = Engine::new();
    let first = r##"{
        "version":1,"sequence":1,"worldId":"lobby","players":[
        {"id":"account:alice","username":"Alice","generation":7,"position":[1,0,2],"yaw":0.2,
         "moving":true,"appearance":{"version":1,"body":"cuba:cat.v1","revision":4}},
        {"id":"account:bob","generation":9,"position":[-1,0,2],"yaw":-0.2,"moving":false}
    ]}"##;
    assert!(engine.apply_remote_update_json(first));
    let first_samples: Vec<_> = engine.character_motion_samples().collect();
    let alice = first_samples
        .iter()
        .find(|sample| {
            sample.key.kind == CharacterEntityKind::RemotePlayer && sample.position[0] == 1.0
        })
        .expect("alice sample");
    let alice_identity = alice.key.identity;
    assert_eq!(alice.key.generation, 7);
    assert_eq!(engine.remote_players[0].display_name, "Alice");
    assert_eq!(
        engine
            .remote_appearance(alice.key)
            .expect("alice appearance")
            .body,
        crate::character::BodyId::Cat
    );

    let second = r##"{
        "version":1,"sequence":2,"worldId":"lobby","players":[
        {"id":"account:bob","generation":9,"position":[-2,0,2],"yaw":-0.3,"moving":true},
        {"id":"account:alice","generation":7,"position":[3,0,2],"yaw":0.4,"moving":false}
    ]}"##;
    assert!(engine.apply_remote_update_json(second));
    let alice_after = engine
        .character_motion_samples()
        .find(|sample| {
            sample.key.kind == CharacterEntityKind::RemotePlayer && sample.position[0] == 3.0
        })
        .expect("reordered alice sample");
    assert_eq!(alice_after.key.identity, alice_identity);
    assert_eq!(alice_after.key.generation, 7);
    assert_eq!(alice_after.appearance_revision, 4);
    assert_eq!(engine.remote_update_sequence(), 2);
}

#[test]
fn remote_updates_reject_stale_sequences_and_deduplicate_emotes() {
    let mut engine = Engine::new();
    let wave = r##"{
        "version":1,"sequence":5,"players":[
        {"id":"account:wave","generation":3,"position":[0,0,0],"yaw":0,
         "emote":"wave","emoteSequence":11}
    ]}"##;
    assert!(engine.apply_remote_update_json(wave));
    assert_eq!(
        engine
            .character_motion_samples()
            .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
            .expect("wave sample")
            .emote,
        crate::types::CharacterEmote::Wave
    );

    let duplicate_emote = r##"{
        "version":1,"sequence":6,"players":[
        {"id":"account:wave","generation":3,"position":[0,0,0],"yaw":0,
         "emote":"wave","emoteSequence":11}
    ]}"##;
    assert!(engine.apply_remote_update_json(duplicate_emote));
    assert_eq!(
        engine
            .character_motion_samples()
            .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
            .expect("deduplicated sample")
            .emote,
        crate::types::CharacterEmote::None
    );

    let stale = r##"{
        "version":1,"sequence":4,"players":[]
    }"##;
    assert!(!engine.apply_remote_update_json(stale));
    assert_eq!(engine.remote_update_status(), 2);
    assert_eq!(engine.remote_player_count(), 1);
}

#[test]
fn remote_duplicate_motion_sequence_does_not_replace_newer_motion() {
    let mut engine = Engine::new();
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":10,"players":[
            {"id":"account:motion","generation":1,"position":[5,0,0],"yaw":0,
             "motionSequence":100}
        ]}"##,
    ));
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":11,"players":[
            {"id":"account:motion","generation":1,"position":[50,0,0],"yaw":1,
             "motionSequence":100}
        ]}"##,
    ));

    let sample = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("remote motion sample");
    assert_eq!(sample.position, [5.0, 0.0, 0.0]);
    assert_eq!(sample.facing_yaw, 0.0);
}

#[test]
fn compact_remote_motion_batch_updates_only_newer_motion() {
    let mut engine = Engine::new();
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"players":[
            {"id":"account:typed-motion","generation":2,"position":[5,0,0],"yaw":0,
             "motionSequence":100}
        ]}"##,
    ));
    let identity = crate::engine::identity::stable_identity("account:typed-motion");
    let mut batch = vec![0_u8; 8 + crate::engine::identity::REMOTE_MOTION_RECORD_BYTES];
    batch[0..4].copy_from_slice(&1_u32.to_le_bytes());
    batch[4..8].copy_from_slice(&1_u32.to_le_bytes());
    batch[8..16].copy_from_slice(&identity.to_le_bytes());
    batch[16..20].copy_from_slice(&2_u32.to_le_bytes());
    batch[20..28].copy_from_slice(&100_u64.to_le_bytes());
    batch[28..32].copy_from_slice(&50.0_f32.to_le_bytes());
    batch[32..36].copy_from_slice(&0.0_f32.to_le_bytes());
    batch[36..40].copy_from_slice(&0.0_f32.to_le_bytes());
    batch[40..44].copy_from_slice(&1.0_f32.to_le_bytes());
    batch[44..48].copy_from_slice(&1_u32.to_le_bytes());
    assert!(engine.apply_remote_motion_batch(&batch));
    let duplicate = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("typed motion sample");
    assert_eq!(duplicate.position, [5.0, 0.0, 0.0]);

    batch[20..28].copy_from_slice(&101_u64.to_le_bytes());
    assert!(engine.apply_remote_motion_batch(&batch));
    let newer = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("newer typed motion sample");
    assert_eq!(newer.position, [50.0, 0.0, 0.0]);
}

#[test]
fn reconnect_can_hydrate_cached_appearance_without_reusing_motion_state() {
    let mut engine = Engine::new();
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":20,"players":[
            {"id":"account:reconnect","generation":4,"position":[8,0,1],"yaw":0.5,
             "appearance":{"version":1,"body":"cuba:dragon.v1","revision":6}}
        ]}"##,
    ));
    let old = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("initial remote");
    assert_eq!(old.key.generation, 4);
    engine.reset_remote_session();

    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"players":[
            {"id":"account:reconnect","generation":5,"position":[-2,0,1],"yaw":-0.5}
        ]}"##,
    ));
    let restored = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("reconnected remote");
    assert_eq!(restored.key.identity, old.key.identity);
    assert_eq!(restored.key.generation, 5);
    assert_eq!(restored.appearance_revision, 6);
    assert_eq!(
        engine
            .remote_appearance(restored.key)
            .expect("cached appearance")
            .body,
        crate::character::BodyId::Dragon
    );
}

#[test]
fn reconnect_rejects_conflicting_same_revision_appearance_from_known_identity() {
    let mut engine = Engine::new();
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"players":[
            {"id":"account:appearance-reconnect","generation":4,"position":[0,0,0],"yaw":0,
             "appearance":{"version":1,"body":"cuba:dragon.v1","revision":6}}
        ]}"##,
    ));
    engine.reset_remote_session();

    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"players":[
            {"id":"account:appearance-reconnect","generation":5,"position":[0,0,0],"yaw":0,
             "appearance":{"version":1,"body":"cuba:cat.v1","revision":6}}
        ]}"##,
    ));
    let restored = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("reconnected remote");
    assert_eq!(engine.remote_update_status(), 3);
    assert_eq!(restored.appearance_revision, 6);
    assert_eq!(
        engine
            .remote_appearance(restored.key)
            .expect("cached appearance")
            .body,
        crate::character::BodyId::Dragon
    );
}

#[test]
fn remote_identity_cache_evicts_least_recently_seen_entry() {
    let mut engine = Engine::new();
    let appearance = crate::character::definition::CharacterAppearance::default();
    for index in 0..MAX_AGENTS {
        engine.cache_remote_appearance(format!("account:{index}"), appearance.clone());
    }
    engine.cache_remote_appearance("account:0".to_owned(), appearance.clone());
    engine.cache_remote_appearance("account:new".to_owned(), appearance);

    assert!(engine.remote_identity_cache.contains_key("account:0"));
    assert!(!engine.remote_identity_cache.contains_key("account:1"));
    assert!(engine.remote_identity_cache.contains_key("account:new"));
}

#[test]
fn first_appearance_after_appearance_omission_is_accepted() {
    let mut engine = Engine::new();
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"players":[
            {"id":"account:first-appearance","generation":1,"position":[0,0,0],"yaw":0}
        ]}"##,
    ));
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":2,"players":[
            {"id":"account:first-appearance","generation":1,"position":[0,0,0],"yaw":0,
             "appearance":{"version":1,"body":"cuba:cat.v1","revision":0}}
        ]}"##,
    ));
    let sample = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("remote with first appearance");
    assert_eq!(
        engine
            .remote_appearance(sample.key)
            .expect("accepted appearance")
            .body,
        crate::character::BodyId::Cat
    );
}

#[test]
fn remote_identity_is_hidden_in_other_world_and_returns_with_same_appearance() {
    let mut engine = Engine::new();
    engine.world_ids = vec!["lobby".to_owned(), "arena".to_owned()];
    engine.worlds = vec![
        crate::world::RuntimeWorld::default(),
        crate::world::RuntimeWorld::default(),
    ];
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"worldId":"lobby","players":[
            {"id":"account:world","generation":8,"position":[1,0,1],"yaw":0,
             "appearance":{"version":1,"body":"cuba:cat.v1","revision":3}}
        ]}"##,
    ));
    let before = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("lobby remote");
    assert!(engine.start_world(1));
    assert!(
        !engine
            .character_motion_samples()
            .any(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
    );
    assert!(engine.start_world(0));
    let after = engine
        .character_motion_samples()
        .find(|sample| sample.key.kind == CharacterEntityKind::RemotePlayer)
        .expect("returning lobby remote");
    assert_eq!(before.key.identity, after.key.identity);
    assert_eq!(before.key.generation, after.key.generation);
    assert_eq!(after.appearance_revision, 3);
}

#[test]
fn remote_missing_content_keeps_a_usable_bundled_fallback() {
    let mut engine = Engine::new();
    assert!(engine.apply_remote_update_json(
        r##"{"version":1,"sequence":1,"players":[
            {"id":"account:missing","generation":1,"position":[0,0,0],"yaw":0,
             "appearance":{"version":1,"body":"cuba:unreleased.v9",
             "outfit":"cuba:missing-coat.v1","revision":2}},
            {"id":"account:legacy","generation":1,"position":[1,0,0],"yaw":0}
        ]}"##,
    ));
    assert_eq!(engine.remote_update_status(), 3);
    let samples: Vec<_> = engine.character_motion_samples().collect();
    let missing = samples
        .iter()
        .find(|sample| {
            sample.key.kind == CharacterEntityKind::RemotePlayer && sample.position[0] == 0.0
        })
        .expect("missing-content remote");
    assert_eq!(
        engine
            .remote_appearance(missing.key)
            .expect("fallback appearance")
            .body,
        crate::character::BodyId::Person
    );
}
