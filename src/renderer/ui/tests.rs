    #[test]
    fn ui_vertices_are_in_clip_space() {
        let frame = UiFrame {
            viewport: UiViewport {
                width: 390.0,
                height: 844.0,
                scale: 1.0,
                safe_area: Default::default(),
            },
            nodes: vec![UiRenderNode {
                id: "button".to_owned(),
                kind: UiNodeKind::Button,
                rect: UiRect {
                    x: 20.0,
                    y: 20.0,
                    width: 100.0,
                    height: 44.0,
                },
                text: "GO".to_owned(),
                icon: None,
                background: Some([0.0, 0.5, 1.0, 1.0]),
                foreground: [1.0; 4],
                border_color: None,
                border_width: 0.0,
                corner_radius: 12.0,
                font_size: 14.0,
                text_align: UiAlignment::Center,
                accent: [0.0, 0.5, 1.0, 1.0],
                image: None,
                image_invert: false,
                value: 0.0,
                value_x: 0.0,
                value_y: 0.0,
                checked: false,
                pressed: false,
                disabled: false,
            }],
        };
        let vertices = build_ui_vertices(&frame);
        assert!(!vertices.is_empty());
        assert!(vertices.iter().all(|vertex| {
            (-1.0..=1.0).contains(&vertex.position[0]) && (-1.0..=1.0).contains(&vertex.position[1])
        }));
    }

    #[test]
    fn world_labels_render_each_glyph_once_with_the_readable_font() {
        let frame = UiFrame {
            viewport: UiViewport {
                width: 390.0,
                height: 844.0,
                scale: 1.0,
                safe_area: Default::default(),
            },
            nodes: Vec::new(),
        };
        let mut vertices = Vec::new();
        add_world_label(&mut vertices, &frame, 195.0, 200.0, "Player1", 16.0);

        let label_atlas_y = WORLD_LABEL_FONT_ATLAS_Y as f32 / UI_ATLAS_HEIGHT as f32;
        let glyph_vertices = vertices
            .iter()
            .filter(|vertex| vertex.tex_coords[1] >= label_atlas_y)
            .count();
        assert_eq!(glyph_vertices, "Player1".chars().count() * 6);
    }
