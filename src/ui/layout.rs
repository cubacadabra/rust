fn validate_document(document: &UiDocument) -> Result<(), String> {
    fn visit(
        nodes: &[UiNode],
        depth: usize,
        count: &mut usize,
        ids: &mut HashSet<String>,
    ) -> Result<(), String> {
        if depth > MAX_UI_DEPTH {
            return Err(format!("UI nesting exceeds {MAX_UI_DEPTH} levels"));
        }
        for node in nodes {
            *count += 1;
            if *count > MAX_UI_NODES {
                return Err(format!("UI document exceeds {MAX_UI_NODES} nodes"));
            }
            if node.id.trim().is_empty() {
                return Err("Every UI node needs a non-empty id".to_owned());
            }
            if !ids.insert(node.id.clone()) {
                return Err(format!("Duplicate UI node id: {}", node.id));
            }
            visit(&node.children, depth + 1, count, ids)?;
        }
        Ok(())
    }

    visit(&document.nodes, 0, &mut 0, &mut HashSet::new())
}

fn find_node_mut<'a>(nodes: &'a mut [UiNode], id: &str) -> Option<&'a mut UiNode> {
    for node in nodes {
        if node.id == id {
            return Some(node);
        }
        if let Some(found) = find_node_mut(&mut node.children, id) {
            return Some(found);
        }
    }
    None
}

fn node_is_visible(node: &UiNode, world_id: &str) -> bool {
    node.visible
        && node
            .visible_in
            .as_ref()
            .is_none_or(|worlds| worlds.iter().any(|world| world == world_id))
}

fn is_persistent_gameplay_control(node: &UiNode) -> bool {
    matches!(
        node.id.as_str(),
        "player-joystick" | "player-jump" | "player-run"
    )
}

fn measure_node(
    node: &UiNode,
    available_width: f32,
    available_height: f32,
    world_id: &str,
) -> (f32, f32) {
    let padding = node.layout.padding.max(0.0) * 2.0;
    let font_size = node.style.font_size.max(7.0);
    let text_width = node
        .text
        .lines()
        .map(|line| line.chars().count() as f32 * font_size * 0.72)
        .fold(0.0, f32::max);
    let text_height = node.text.lines().count().max(1) as f32 * font_size * 1.25;
    let child_sizes = node
        .children
        .iter()
        .filter(|child| node_is_visible(child, world_id))
        .map(|child| measure_node(child, available_width, available_height, world_id))
        .collect::<Vec<_>>();
    let gap = node.layout.gap.max(0.0) * child_sizes.len().saturating_sub(1) as f32;
    let (children_width, children_height) = match node.layout.direction {
        UiDirection::Row => (
            child_sizes.iter().map(|size| size.0).sum::<f32>() + gap,
            child_sizes.iter().map(|size| size.1).fold(0.0, f32::max),
        ),
        UiDirection::Column => (
            child_sizes.iter().map(|size| size.0).fold(0.0, f32::max),
            child_sizes.iter().map(|size| size.1).sum::<f32>() + gap,
        ),
    };
    let leaf_width = match node.kind {
        UiNodeKind::Toggle => 72.0,
        UiNodeKind::Slider => 180.0,
        UiNodeKind::Joystick => 120.0,
        UiNodeKind::Button if node.icon.is_some() && node.text.is_empty() => MIN_TOUCH_TARGET,
        UiNodeKind::Button => text_width.max(MIN_TOUCH_TARGET),
        _ => text_width,
    };
    let leaf_height = match node.kind {
        UiNodeKind::Joystick => 120.0,
        UiNodeKind::Button | UiNodeKind::Toggle | UiNodeKind::Slider => MIN_TOUCH_TARGET,
        _ => text_height,
    };
    let intrinsic_width = leaf_width.max(children_width) + padding;
    let intrinsic_height = if node.children.is_empty() {
        leaf_height + padding
    } else {
        children_height.max(if node.text.is_empty() {
            0.0
        } else {
            leaf_height
        }) + padding
    };
    let minimum = if matches!(
        node.kind,
        UiNodeKind::Button | UiNodeKind::Toggle | UiNodeKind::Slider | UiNodeKind::Joystick
    ) {
        MIN_TOUCH_TARGET
    } else {
        0.0
    };
    (
        clamp_length(
            node.layout
                .width
                .resolve(available_width, intrinsic_width)
                .max(minimum.min(available_width)),
            node.layout.max_width,
            available_width,
        ),
        clamp_length(
            node.layout
                .height
                .resolve(available_height, intrinsic_height)
                .max(minimum.min(available_height)),
            node.layout.max_height,
            available_height,
        ),
    )
}

fn clamp_length(value: f32, maximum: Option<f32>, available: f32) -> f32 {
    value
        .min(maximum.unwrap_or(f32::MAX).max(0.0))
        .min(available.max(0.0))
}

fn anchored_rect(
    parent: UiRect,
    width: f32,
    height: f32,
    anchor: UiAnchor,
    offset: [f32; 2],
) -> UiRect {
    let horizontal = match anchor {
        UiAnchor::Top | UiAnchor::Center | UiAnchor::Bottom => 0.5,
        UiAnchor::TopRight | UiAnchor::Right | UiAnchor::BottomRight => 1.0,
        _ => 0.0,
    };
    let vertical = match anchor {
        UiAnchor::Left | UiAnchor::Center | UiAnchor::Right => 0.5,
        UiAnchor::BottomLeft | UiAnchor::Bottom | UiAnchor::BottomRight => 1.0,
        _ => 0.0,
    };
    let x = parent.x + (parent.width - width) * horizontal + offset[0];
    let y = parent.y + (parent.height - height) * vertical + offset[1];
    UiRect {
        x: x.clamp(parent.x, (parent.x + parent.width - width).max(parent.x)),
        y: y.clamp(parent.y, (parent.y + parent.height - height).max(parent.y)),
        width,
        height,
    }
}
fn layout_region_roots(
    roots: &[&UiNode],
    parent: UiRect,
    centered: bool,
    world_id: &str,
    pressed: &HashSet<&str>,
    nodes: &mut Vec<UiRenderNode>,
    hit_regions: &mut Vec<UiHitRegion>,
) {
    if roots.is_empty() || parent.width <= 0.0 || parent.height <= 0.0 {
        return;
    }

    let mut sizes = roots
        .iter()
        .map(|node| {
            let intrinsic = measure_node(node, parent.width, parent.height, world_id);
            (
                clamp_length(
                    node.layout.width.resolve(parent.width, intrinsic.0),
                    node.layout.max_width,
                    parent.width,
                ),
                clamp_length(
                    node.layout.height.resolve(parent.height, intrinsic.1),
                    node.layout.max_height,
                    parent.height,
                ),
            )
        })
        .collect::<Vec<_>>();
    let gap = SHARED_HEADER_CELL_GAP;
    let requested_width =
        sizes.iter().map(|size| size.0).sum::<f32>() + gap * roots.len().saturating_sub(1) as f32;
    if requested_width > parent.width && !sizes.is_empty() {
        let available = (parent.width - gap * roots.len().saturating_sub(1) as f32).max(0.0);
        let requested = sizes.iter().map(|size| size.0).sum::<f32>().max(1.0);
        for size in &mut sizes {
            size.0 = size.0 * available / requested;
        }
    }
    let total_width =
        sizes.iter().map(|size| size.0).sum::<f32>() + gap * roots.len().saturating_sub(1) as f32;
    let mut cursor = if centered {
        (parent.width - total_width).max(0.0) * 0.5
    } else {
        0.0
    };

    for (node, (width, height)) in roots.iter().zip(sizes) {
        let rect = UiRect {
            x: parent.x + cursor + node.layout.offset[0],
            y: parent.y + (parent.height - height).max(0.0) * 0.5 + node.layout.offset[1],
            width,
            height,
        };
        layout_node(node, rect, world_id, pressed, nodes, hit_regions);
        cursor += width + gap;
    }
}

fn layout_node(
    node: &UiNode,
    rect: UiRect,
    world_id: &str,
    pressed: &HashSet<&str>,
    nodes: &mut Vec<UiRenderNode>,
    hit_regions: &mut Vec<UiHitRegion>,
) {
    if !node_is_visible(node, world_id) {
        return;
    }
    let value = match node.kind {
        UiNodeKind::Toggle => f32::from(node.checked),
        UiNodeKind::Slider => ((node.value - node.minimum)
            / (node.maximum - node.minimum).max(f32::EPSILON))
        .clamp(0.0, 1.0),
        _ => node.value,
    };
    nodes.push(UiRenderNode {
        id: node.id.clone(),
        kind: node.kind,
        rect,
        text: node.text.clone(),
        icon: node.icon.clone(),
        background: node.style.background.as_deref().and_then(parse_color),
        foreground: parse_color(&node.style.foreground).unwrap_or([1.0; 4]),
        border_color: node.style.border_color.as_deref().and_then(parse_color),
        border_width: node.style.border_width.max(0.0),
        corner_radius: node.style.corner_radius.max(0.0),
        font_size: node.style.font_size.max(7.0),
        text_align: node.style.text_align,
        accent: node
            .style
            .accent
            .as_deref()
            .and_then(parse_color)
            .unwrap_or([0.0, 0.58, 1.0, 1.0]),
        image: None,
        image_invert: false,
        value,
        value_x: node.value_x,
        value_y: node.value_y,
        checked: node.checked,
        pressed: pressed.contains(node.id.as_str()),
        disabled: node.disabled,
    });
    let inherently_interactive = matches!(
        node.kind,
        UiNodeKind::Button | UiNodeKind::Toggle | UiNodeKind::Slider | UiNodeKind::Joystick
    );
    if inherently_interactive || node.blocks_input || !node.action.is_empty() {
        hit_regions.push(UiHitRegion {
            id: node.id.clone(),
            action: if node.action.is_empty() && inherently_interactive {
                node.id.clone()
            } else {
                node.action.clone()
            },
            kind: node.kind,
            rect,
            disabled: node.disabled,
        });
    }
    if node.children.is_empty() {
        return;
    }

    let content = rect.inset(node.layout.padding);
    let visible_children = node
        .children
        .iter()
        .filter(|child| node_is_visible(child, world_id))
        .collect::<Vec<_>>();
    if visible_children.is_empty() {
        return;
    }
    let gap = node.layout.gap.max(0.0);
    let main_available = match node.layout.direction {
        UiDirection::Row => content.width,
        UiDirection::Column => content.height,
    };
    let cross_available = match node.layout.direction {
        UiDirection::Row => content.height,
        UiDirection::Column => content.width,
    };
    let intrinsic = visible_children
        .iter()
        .map(|child| measure_node(child, content.width, content.height, world_id))
        .collect::<Vec<_>>();
    let fill_count = visible_children
        .iter()
        .filter(|child| match node.layout.direction {
            UiDirection::Row => child.layout.width.is_fill(),
            UiDirection::Column => child.layout.height.is_fill(),
        })
        .count();
    let fixed_main = visible_children
        .iter()
        .zip(&intrinsic)
        .filter(|(child, _)| match node.layout.direction {
            UiDirection::Row => !child.layout.width.is_fill(),
            UiDirection::Column => !child.layout.height.is_fill(),
        })
        .map(|(_, size)| match node.layout.direction {
            UiDirection::Row => size.0,
            UiDirection::Column => size.1,
        })
        .sum::<f32>();
    let base_gap = gap * visible_children.len().saturating_sub(1) as f32;
    let fill_main = if fill_count > 0 {
        ((main_available - fixed_main - base_gap).max(0.0)) / fill_count as f32
    } else {
        0.0
    };
    let used_main = if fill_count > 0 {
        main_available
    } else {
        fixed_main
            + intrinsic
                .iter()
                .enumerate()
                .filter_map(|(index, size)| {
                    let child = visible_children[index];
                    let is_fill = match node.layout.direction {
                        UiDirection::Row => child.layout.width.is_fill(),
                        UiDirection::Column => child.layout.height.is_fill(),
                    };
                    is_fill.then_some(match node.layout.direction {
                        UiDirection::Row => size.0,
                        UiDirection::Column => size.1,
                    })
                })
                .sum::<f32>()
            + base_gap
    };
    let mut actual_gap = gap;
    let mut cursor = match node.layout.justify {
        UiJustification::Center => (main_available - used_main).max(0.0) * 0.5,
        UiJustification::End => (main_available - used_main).max(0.0),
        UiJustification::SpaceBetween if visible_children.len() > 1 && fill_count == 0 => {
            actual_gap = ((main_available - (used_main - base_gap)).max(0.0))
                / visible_children.len().saturating_sub(1) as f32;
            0.0
        }
        _ => 0.0,
    };

    for (child, intrinsic) in visible_children.into_iter().zip(intrinsic) {
        let main_is_fill = match node.layout.direction {
            UiDirection::Row => child.layout.width.is_fill(),
            UiDirection::Column => child.layout.height.is_fill(),
        };
        let main = if main_is_fill {
            fill_main
        } else {
            match node.layout.direction {
                UiDirection::Row => intrinsic.0,
                UiDirection::Column => intrinsic.1,
            }
        };
        let child_cross_fill = match node.layout.direction {
            UiDirection::Row => child.layout.height.is_fill(),
            UiDirection::Column => child.layout.width.is_fill(),
        } || node.layout.align == UiAlignment::Stretch;
        let cross = if child_cross_fill {
            cross_available
        } else {
            match node.layout.direction {
                UiDirection::Row => intrinsic.1.min(cross_available),
                UiDirection::Column => intrinsic.0.min(cross_available),
            }
        };
        let cross_offset = match node.layout.align {
            UiAlignment::Center => (cross_available - cross) * 0.5,
            UiAlignment::End => cross_available - cross,
            _ => 0.0,
        };
        let child_rect = match node.layout.direction {
            UiDirection::Row => UiRect {
                x: content.x + cursor + child.layout.offset[0],
                y: content.y + cross_offset + child.layout.offset[1],
                width: main,
                height: cross,
            },
            UiDirection::Column => UiRect {
                x: content.x + cross_offset + child.layout.offset[0],
                y: content.y + cursor + child.layout.offset[1],
                width: cross,
                height: main,
            },
        };
        layout_node(child, child_rect, world_id, pressed, nodes, hit_regions);
        cursor += main + actual_gap;
    }
}

pub(crate) fn parse_color(value: &str) -> Option<[f32; 4]> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 && value.len() != 8 {
        return None;
    }
    let color = u32::from_str_radix(value, 16).ok()?;
    let (rgb, alpha) = if value.len() == 8 {
        (color >> 8, (color & 0xff) as f32 / 255.0)
    } else {
        (color, 1.0)
    };
    Some([
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
        alpha,
    ])
}

