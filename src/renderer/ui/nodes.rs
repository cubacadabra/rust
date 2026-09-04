pub(super) fn build_ui_vertices(frame: &UiFrame) -> Vec<Vertex> {
    if frame.viewport.width <= 0.0 || frame.viewport.height <= 0.0 {
        #[cfg(target_os = "ios")]
        log_ui_draw_vertex_count(0);
        return Vec::new();
    }
    let mut vertices = Vec::with_capacity(frame.nodes.len() * 96);
    for node in &frame.nodes {
        if node.pressed {
            let mut pressed = node.clone();
            pressed.rect.y += 2.0;
            add_node(&mut vertices, frame, &pressed);
        } else {
            add_node(&mut vertices, frame, node);
        }
    }
    #[cfg(target_os = "ios")]
    log_ui_draw_vertex_count(vertices.len());
    vertices
}

fn add_node(vertices: &mut Vec<Vertex>, frame: &UiFrame, node: &UiRenderNode) {
    let mut opacity = if node.disabled { 0.48 } else { 1.0 };
    if node.pressed {
        opacity *= 0.78;
    }

    let has_surface = node.background.is_some() || node.border_color.is_some();
    let is_control = matches!(
        node.kind,
        UiNodeKind::Button | UiNodeKind::Toggle | UiNodeKind::Slider
    );
    let automatic_border = is_control && node.border_color.is_none() && node.background.is_some();
    let border_width = if node.border_color.is_some() || automatic_border {
        node.border_width.max(1.0)
    } else {
        node.border_width
    };

    // A quiet, close shadow keeps translucent controls legible against the 3D
    // world. It is deliberately tight so the HUD still feels light on mobile.
    if has_surface {
        add_rounded_rect(
            vertices,
            frame,
            UiRect {
                y: node.rect.y + 3.0,
                ..node.rect
            },
            node.corner_radius,
            faded([0.01, 0.04, 0.05, 0.25], opacity),
        );
    }

    if let Some(border) = node
        .border_color
        .or_else(|| automatic_border.then_some([0.82, 0.94, 0.94, 0.12]))
    {
        add_rounded_rect(
            vertices,
            frame,
            node.rect,
            node.corner_radius,
            faded(border, opacity),
        );
    }
    if let Some(background) = node.background {
        let inset = border_width.min(node.rect.width.min(node.rect.height) * 0.5);
        add_rounded_rect(
            vertices,
            frame,
            inset_rect(node.rect, inset),
            (node.corner_radius - inset).max(0.0),
            faded(background, opacity),
        );

        // A narrow upper sheen gives the dark surfaces a material edge and
        // makes the primary controls read as intentional interactive objects.
        if node.rect.height >= 28.0 {
            let highlight = inset_rect(node.rect, border_width + 1.0);
            add_rounded_rect(
                vertices,
                frame,
                UiRect {
                    height: (highlight.height * 0.34).max(2.0),
                    ..highlight
                },
                (node.corner_radius - border_width - 1.0).max(0.0),
                faded([0.92, 0.98, 0.97, 0.075], opacity),
            );
        }
    }

    // Checked is the shared selected state for buttons, swatches, tabs, and
    // toggles. The light keyline plus accent ring stays readable on any fill.
    if node.checked && matches!(node.kind, UiNodeKind::Button | UiNodeKind::Toggle) {
        add_rounded_rect(
            vertices,
            frame,
            node.rect,
            node.corner_radius,
            faded([0.96, 0.98, 0.94, 1.0], opacity),
        );
        add_rounded_rect(
            vertices,
            frame,
            inset_rect(node.rect, 2.0),
            (node.corner_radius - 2.0).max(0.0),
            faded(node.accent, opacity),
        );
        if let Some(background) = node.background {
            add_rounded_rect(
                vertices,
                frame,
                inset_rect(node.rect, 5.0),
                (node.corner_radius - 5.0).max(0.0),
                faded(background, opacity),
            );
        }
    }

    if let Some(image) = node.image {
        add_image(
            vertices,
            frame,
            node.rect,
            image,
            node.image_invert,
            opacity,
        );
    }

    if node.pressed && has_surface {
        add_rounded_rect(
            vertices,
            frame,
            inset_rect(node.rect, border_width),
            (node.corner_radius - border_width).max(0.0),
            faded([0.01, 0.03, 0.04, 0.10], opacity),
        );
    }

    if let Some(icon) = node.icon.as_deref() {
        let icon_rect = if node.text.is_empty() {
            node.rect
        } else {
            UiRect {
                height: node.rect.height * 0.52,
                ..node.rect
            }
        };
        add_icon(
            vertices,
            frame,
            icon,
            icon_rect,
            faded(node.foreground, opacity),
        );
    }

    match node.kind {
        UiNodeKind::Toggle => add_toggle(vertices, frame, node, opacity),
        UiNodeKind::Slider => add_slider(vertices, frame, node, opacity),
        UiNodeKind::Joystick => add_joystick(vertices, frame, node, opacity),
        _ => {}
    }
    if !node.text.is_empty() {
        let text_rect = match node.kind {
            UiNodeKind::Toggle => UiRect {
                width: (node.rect.width - 68.0).max(0.0),
                ..node.rect
            },
            _ if node.icon.is_some() => UiRect {
                y: node.rect.y + node.rect.height * 0.46,
                height: node.rect.height * 0.54,
                ..node.rect
            },
            _ => node.rect,
        };
        add_text(
            vertices,
            frame,
            &node.text,
            text_rect,
            node.font_size,
            node.text_align,
            faded(node.foreground, opacity),
        );
    }
    if node.checked
        && node.kind == UiNodeKind::Button
        && node.rect.width >= 50.0
        && node.rect.height >= 50.0
    {
        let center = (node.rect.x + node.rect.width - 10.0, node.rect.y + 10.0);
        add_circle(
            vertices,
            frame,
            center.0,
            center.1,
            8.0,
            faded([0.97, 0.99, 0.95, 1.0], opacity),
        );
        let ink = faded([0.02, 0.12, 0.18, 1.0], opacity);
        add_line(
            vertices,
            frame,
            (center.0 - 4.0, center.1),
            (center.0 - 1.0, center.1 + 3.0),
            2.2,
            ink,
        );
        add_line(
            vertices,
            frame,
            (center.0 - 1.0, center.1 + 3.0),
            (center.0 + 4.0, center.1 - 3.0),
            2.2,
            ink,
        );
    }
}

fn add_joystick(vertices: &mut Vec<Vertex>, frame: &UiFrame, node: &UiRenderNode, opacity: f32) {
    let radius = node.rect.width.min(node.rect.height) * 0.5;
    let center_x = node.rect.x + node.rect.width * 0.5;
    let center_y = node.rect.y + node.rect.height * 0.5;
    add_circle(
        vertices,
        frame,
        center_x,
        center_y,
        radius,
        faded([0.04, 0.08, 0.09, 0.52], opacity),
    );
    add_circle(
        vertices,
        frame,
        center_x,
        center_y,
        (radius - 2.0).max(0.0),
        faded([0.08, 0.14, 0.15, 0.76], opacity),
    );
    add_circle(
        vertices,
        frame,
        center_x,
        center_y,
        (radius - 7.0).max(0.0),
        faded([0.11, 0.19, 0.20, 0.28], opacity),
    );
    let travel = radius * 0.46;
    let knob_x = center_x + node.value_x.clamp(-1.0, 1.0) * travel;
    let knob_y = center_y + node.value_y.clamp(-1.0, 1.0) * travel;
    add_circle(
        vertices,
        frame,
        knob_x,
        knob_y + 2.0,
        radius * 0.235,
        faded([0.01, 0.04, 0.05, 0.32], opacity),
    );
    add_circle(
        vertices,
        frame,
        knob_x,
        knob_y,
        radius * 0.21,
        faded(node.foreground, opacity),
    );
}

fn add_toggle(vertices: &mut Vec<Vertex>, frame: &UiFrame, node: &UiRenderNode, opacity: f32) {
    let track = UiRect {
        x: node.rect.x + node.rect.width - 50.0,
        y: node.rect.y + (node.rect.height - 28.0) * 0.5,
        width: 48.0,
        height: 28.0,
    };
    let off = [0.34, 0.38, 0.40, 0.9];
    add_rounded_rect(
        vertices,
        frame,
        track,
        14.0,
        faded(if node.value >= 0.5 { node.accent } else { off }, opacity),
    );
    let thumb_x = if node.value >= 0.5 {
        track.x + track.width - 14.0
    } else {
        track.x + 14.0
    };
    add_circle(
        vertices,
        frame,
        thumb_x,
        track.y + track.height * 0.5 + 1.0,
        10.5,
        faded([0.01, 0.04, 0.05, 0.30], opacity),
    );
    add_circle(
        vertices,
        frame,
        thumb_x,
        track.y + track.height * 0.5,
        10.0,
        faded([1.0, 1.0, 1.0, 1.0], opacity),
    );
}

fn add_slider(vertices: &mut Vec<Vertex>, frame: &UiFrame, node: &UiRenderNode, opacity: f32) {
    let track = UiRect {
        x: node.rect.x + 12.0,
        y: node.rect.y + node.rect.height * 0.5 - 3.0,
        width: (node.rect.width - 24.0).max(1.0),
        height: 6.0,
    };
    add_rounded_rect(
        vertices,
        frame,
        track,
        3.0,
        faded([0.35, 0.39, 0.41, 0.9], opacity),
    );
    let fill = UiRect {
        width: track.width * node.value.clamp(0.0, 1.0),
        ..track
    };
    add_rounded_rect(vertices, frame, fill, 3.0, faded(node.accent, opacity));
    add_circle(
        vertices,
        frame,
        track.x + track.width * node.value.clamp(0.0, 1.0),
        track.y + track.height * 0.5 + 1.0,
        10.5,
        faded([0.01, 0.04, 0.05, 0.30], opacity),
    );
    add_circle(
        vertices,
        frame,
        track.x + track.width * node.value.clamp(0.0, 1.0),
        track.y + track.height * 0.5,
        10.0,
        faded([1.0, 1.0, 1.0, 1.0], opacity),
    );
}

fn add_text(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    text: &str,
    rect: UiRect,
    font_size: f32,
    alignment: UiAlignment,
    color: [f32; 4],
) {
    let font_size = font_size.max(7.0);
    let lines = text
        .lines()
        .take(8)
        .map(|line| {
            let glyphs = line
                .chars()
                .filter(|character| character.is_ascii())
                .map(|character| {
                    let character = character.to_ascii_uppercase();
                    let glyph = ui_atlas_glyphs()
                        .iter()
                        .find(|glyph| glyph.character == character)
                        .or_else(|| {
                            ui_atlas_glyphs()
                                .iter()
                                .find(|glyph| glyph.character == '?')
                        })
                        .expect("UI atlas includes a fallback glyph");
                    UiGlyph { glyph }
                })
                .take(96)
                .collect::<Vec<_>>();
            let scale = font_size / UI_FONT_ATLAS_SIZE;
            let width = glyphs
                .iter()
                .map(|glyph| glyph.glyph.metrics.advance_width * scale)
                .sum();
            UiTextLine { glyphs, width }
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return;
    }

    let line_height = ui_font()
        .horizontal_line_metrics(font_size)
        .map(|metrics| metrics.new_line_size)
        .unwrap_or(font_size * 1.2)
        .max(font_size * 1.1);
    let block_height = lines.len() as f32 * line_height;
    let first_baseline = rect.y
        + (rect.height - block_height).max(0.0) * 0.5
        + ui_font()
            .horizontal_line_metrics(font_size)
            .map_or(font_size * 0.82, |metrics| metrics.ascent);

    // A compact shadow and proportional outline keep small labels crisp while
    // retaining the friendly game-mark character at larger sizes.
    let outline = (font_size * 0.11).clamp(0.8, 2.0);
    add_raster_text(
        vertices,
        frame,
        &lines,
        rect,
        first_baseline,
        line_height,
        alignment,
        [0.01, 0.04, 0.05, color[3] * 0.48],
        [outline * 0.9, outline * 1.5],
    );
    for offset in [
        [-outline * 0.9, -outline * 0.7],
        [0.0, -outline],
        [outline * 0.9, -outline * 0.7],
        [-outline, 0.0],
        [outline, 0.0],
        [-outline * 0.8, outline * 0.75],
        [0.0, outline],
        [outline * 0.8, outline * 0.75],
    ] {
        add_raster_text(
            vertices,
            frame,
            &lines,
            rect,
            first_baseline,
            line_height,
            alignment,
            [0.01, 0.025, 0.03, color[3] * 0.90],
            offset,
        );
    }
    add_raster_text(
        vertices,
        frame,
        &lines,
        rect,
        first_baseline,
        line_height,
        alignment,
        color,
        [0.0, 0.0],
    );
}

struct UiGlyph {
    glyph: &'static UiAtlasGlyph,
}

struct UiTextLine {
    glyphs: Vec<UiGlyph>,
    width: f32,
}

#[allow(clippy::too_many_arguments)]
fn add_raster_text(
    vertices: &mut Vec<Vertex>,
    frame: &UiFrame,
    lines: &[UiTextLine],
    rect: UiRect,
    first_baseline: f32,
    line_height: f32,
    alignment: UiAlignment,
    color: [f32; 4],
    offset: [f32; 2],
) {
    let atlas_line_height = ui_font()
        .horizontal_line_metrics(UI_FONT_ATLAS_SIZE)
        .map(|metrics| metrics.new_line_size)
        .unwrap_or(UI_FONT_ATLAS_SIZE * 1.2)
        .max(UI_FONT_ATLAS_SIZE * 1.1);
    let scale = line_height / atlas_line_height;
    for (line_index, line) in lines.iter().enumerate() {
        let line_width = line.width;
        let start_x = match alignment {
            UiAlignment::Center | UiAlignment::Stretch => {
                rect.x + (rect.width - line_width).max(0.0) * 0.5
            }
            UiAlignment::End => rect.x + (rect.width - line_width).max(0.0),
            UiAlignment::Start => rect.x,
        };
        let mut cursor_x = start_x + offset[0];
        let baseline = first_baseline + line_index as f32 * line_height + offset[1];
        for glyph in &line.glyphs {
            let metrics = &glyph.glyph.metrics;
            let top = baseline - metrics.ymin as f32 * scale - metrics.height as f32 * scale;
            let left = cursor_x + metrics.xmin as f32 * scale;
            if metrics.width > 0 && metrics.height > 0 {
                add_atlas_rect(
                    vertices,
                    frame,
                    UiRect {
                        x: left,
                        y: top,
                        width: metrics.width as f32 * scale,
                        height: metrics.height as f32 * scale,
                    },
                    [
                        glyph.glyph.x as f32 / UI_ATLAS_WIDTH as f32,
                        UI_FONT_ATLAS_Y as f32 / UI_ATLAS_HEIGHT as f32,
                        (glyph.glyph.x + metrics.width as u32) as f32 / UI_ATLAS_WIDTH as f32,
                        (UI_FONT_ATLAS_Y + metrics.height as u32) as f32 / UI_ATLAS_HEIGHT as f32,
                    ],
                    color,
                );
            }
            cursor_x += metrics.advance_width * scale;
        }
    }
}


