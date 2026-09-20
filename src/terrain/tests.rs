use super::*;

fn operation(
    kind: &str,
    shape: &str,
    position: [f32; 3],
    size: [f32; 3],
    radius: f32,
    material: &str,
) -> TerrainOperationDefinition {
    TerrainOperationDefinition {
        shape: shape.into(),
        operation: kind.into(),
        position: position.to_vec(),
        size: size.to_vec(),
        radius,
        material: material.into(),
    }
}

fn terrain(operations: Vec<TerrainOperationDefinition>) -> TerrainGrid {
    TerrainGrid::build(&TerrainDefinition {
        operations,
        ..TerrainDefinition::default()
    })
    .unwrap()
    .unwrap()
}

#[test]
fn material_art_is_optional_and_does_not_change_terrain_storage() {
    assert!(TerrainDefinition::default().material_art);
    let definition: TerrainDefinition = serde_json::from_str(
        r##"{
            "materialArt":false,
            "operations":[{
                "shape":"block","operation":"fill","position":[0,0,0],
                "size":[2,2,2],"material":"builtin:grass"
            }]
        }"##,
    )
    .expect("materialArt should be a visual-only terrain option");
    assert!(!definition.material_art);
    let terrain = TerrainGrid::build(&definition)
        .expect("terrain mechanics must work without art")
        .expect("the fill should produce terrain");
    assert!(terrain.signed_distance([0.0; 3]) < 0.0);
}

#[test]
fn sparse_chunks_share_one_signed_field_for_surface_and_collision() {
    let terrain = terrain(vec![operation(
        "fill",
        "block",
        [0.0, -1.0, 0.0],
        [20.0, 2.0, 20.0],
        0.0,
        "builtin:grass",
    )]);
    assert!(terrain.chunks_len() < 40);
    assert!(terrain.sample_count() < 40 * SAMPLE_COUNT);
    assert!(terrain.signed_distance([0.0, -1.0, 0.0]) < 0.0);
    assert!(terrain.signed_distance([0.0, 0.2, 0.0]) > 0.0);
    assert!(terrain.capsule_clear([0.0, 0.0, 0.0], 0.5, 2.0));
    assert!(!terrain.capsule_clear([0.0, -0.4, 0.0], 0.5, 2.0));
}

#[test]
fn sphere_sweep_finds_terrain_before_its_center_enters_the_wall() {
    let terrain = terrain(vec![operation(
        "fill",
        "block",
        [0.0, 2.0, 4.0],
        [4.0, 4.0, 1.0],
        0.0,
        "builtin:grass",
    )]);
    let hit = terrain
        .sweep_sphere([0.0, 2.0, 0.0], [0.0, 2.0, 8.0], 0.35)
        .expect("the camera sphere should hit the terrain wall");
    assert!(hit > 3.0 && hit < 3.5, "unexpected sweep distance: {hit}");
}

#[test]
fn ellipsoid_is_a_reusable_volume_for_rounded_terrain_bodies() {
    let terrain = terrain(vec![operation(
        "fill",
        "ellipsoid",
        [0.0, -2.0, 0.0],
        [12.0, 6.0, 8.0],
        0.0,
        "ground",
    )]);
    assert!(terrain.signed_distance([0.0, -2.0, 0.0]) < 0.0);
    assert!(terrain.signed_distance([0.0, 1.1, 0.0]) > 0.0);
    assert!(terrain.signed_distance([6.1, -2.0, 0.0]) > 0.0);
    assert!(terrain.signed_distance([0.0, -2.0, 0.0]) < 0.0);
}

#[test]
fn allocated_bounds_cover_negative_sparse_and_carved_terrain() {
    let terrain = terrain(vec![
        operation(
            "fill",
            "block",
            [-20.0, -2.0, -12.0],
            [4.0, 4.0, 4.0],
            0.0,
            "ground",
        ),
        operation("carve", "ball", [-20.0, -2.0, -12.0], [0.0; 3], 1.0, "air"),
    ]);
    let (low, high) = terrain
        .allocated_bounds()
        .expect("the fill allocates a sparse terrain region");
    assert!(low[0] <= -20.0 && low[2] <= -12.0);
    assert!(high[0] >= -18.0 && high[2] >= -10.0);
    assert!(terrain.signed_distance([-20.0, -2.0, -12.0]) > 0.0);
    assert!(terrain.chunks_len() < 16);
}

#[test]
fn carve_and_paint_are_ordered_volume_operations() {
    let terrain = terrain(vec![
        operation("fill", "block", [0.0; 3], [12.0; 3], 0.0, "ground"),
        operation("carve", "ball", [0.0; 3], [0.0; 3], 2.5, "air"),
        operation("paint", "ball", [4.0, 0.0, 0.0], [0.0; 3], 1.5, "rock"),
    ]);
    assert!(terrain.signed_distance([0.0; 3]) > 0.0);
    assert!(terrain.signed_distance([4.0, 0.0, 0.0]) < 0.0);
    let mut triangles = 0;
    terrain.for_each_chunk_triangle(|_, triangle| {
        triangles += 1;
        assert!(
            triangle
                .iter()
                .all(|vertex| vertex.normal.iter().all(|v| v.is_finite()))
        );
    });
    assert!(triangles > 0);
}

#[test]
fn rejects_unknown_materials_and_unbounded_chunk_grids() {
    let unknown = TerrainDefinition {
        operations: vec![operation("fill", "block", [0.0; 3], [2.0; 3], 0.0, "lava")],
        ..TerrainDefinition::default()
    };
    assert!(
        TerrainGrid::build(&unknown)
            .unwrap_err()
            .contains("unsupported")
    );

    let too_large = TerrainDefinition {
        operations: vec![operation(
            "fill",
            "block",
            [0.0; 3],
            [4096.0; 3],
            0.0,
            "grass",
        )],
        ..TerrainDefinition::default()
    };
    assert!(
        TerrainGrid::build(&too_large)
            .unwrap_err()
            .contains("chunk")
    );
}

#[test]
fn fill_and_sphere_carve_form_a_walkable_tunnel_across_chunk_edges() {
    let terrain = terrain(vec![
        operation(
            "fill",
            "block",
            [0.0, -2.0, 0.0],
            [64.0, 4.0, 64.0],
            0.0,
            "grass",
        ),
        operation("carve", "ball", [15.5, 0.0, 0.0], [0.0; 3], 2.0, "air"),
    ]);
    assert!(terrain.chunks_len() > 1);
    assert!(terrain.signed_distance([15.5, 0.0, 0.0]) > 0.0);
    assert!(terrain.signed_distance([15.5, -3.9, 0.0]) < 0.0);
    let mut crossing_triangles = 0;
    terrain.for_each_chunk_triangle(|_, triangle| {
        if triangle
            .iter()
            .any(|vertex| (vertex.position[0] - 16.0).abs() < 0.02)
        {
            crossing_triangles += 1;
        }
    });
    assert!(crossing_triangles > 0);
}

#[test]
fn flat_walkable_surface_remains_clear_across_chunk_edges() {
    let terrain = TerrainGrid::build(&TerrainDefinition {
        cell_size: 2.0,
        operations: vec![operation(
            "fill",
            "block",
            [0.0, -1.0, 0.0],
            [62.0, 2.0, 6.0],
            0.0,
            "ground",
        )],
        ..TerrainDefinition::default()
    })
    .expect("valid terrain")
    .expect("filled terrain");

    for step in -120..=120 {
        let x = step as f32 * 0.25;
        assert!(
            terrain.capsule_clear([x, 0.0, 0.0], 0.52, 3.2),
            "flat terrain blocked the player capsule at x={x}"
        );
    }
}

#[test]
fn coplanar_walkable_fills_do_not_create_capsule_blocking_ridges() {
    let terrain = TerrainGrid::build(&TerrainDefinition {
        cell_size: 2.0,
        operations: vec![
            operation(
                "fill",
                "block",
                [0.0, -1.0, 0.0],
                [28.0, 2.0, 20.0],
                0.0,
                "grass",
            ),
            operation(
                "fill",
                "block",
                [-20.5, -1.0, 2.0],
                [21.0, 2.0, 3.0],
                0.0,
                "ground",
            ),
        ],
        ..TerrainDefinition::default()
    })
    .expect("valid terrain")
    .expect("filled terrain");

    for step in -120..=-36 {
        let x = step as f32 * 0.25;
        assert!(
            terrain.capsule_clear([x, -0.02, 2.0], 0.52, 3.2),
            "coplanar terrain blocked the player capsule at x={x}"
        );
    }
}

#[test]
fn rejects_features_smaller_than_the_voxel_sampling_resolution() {
    let undersampled = TerrainDefinition {
        cell_size: 1.0,
        operations: vec![operation(
            "fill",
            "block",
            [0.0; 3],
            [2.0, 0.5, 2.0],
            0.0,
            "grass",
        )],
        ..TerrainDefinition::default()
    };
    assert!(
        TerrainGrid::build(&undersampled)
            .unwrap_err()
            .contains("cellSize")
    );
}
