const SHARED_MODAL_ANIMATION_SECONDS: f32 = 0.22;
const SHARED_HEADER_SIZE: f32 = 56.0;
const SHARED_HEADER_MARGIN: f32 = 24.0;
const SHARED_HEADER_GAP: f32 = 12.0;
const SHARED_HEADER_CELL_GAP: f32 = 8.0;
const SHARED_HEADER_PILL_PADDING: f32 = 14.0;
const REGION_CONTROL_HEIGHT: f32 = 68.0;

struct SharedHeaderGeometry {
    nodes: Vec<UiRenderNode>,
    custom_x: f32,
    y: f32,
    size: f32,
}

fn shared_header_nodes(viewport: UiViewport, safe: UiRect) -> SharedHeaderGeometry {
    if viewport.width <= 0.0 || viewport.height <= 0.0 {
        return SharedHeaderGeometry {
            nodes: Vec::new(),
            custom_x: safe.x,
            y: safe.y,
            size: 0.0,
        };
    }

    // Safe-area insets make a 4:3 iPad portrait window slightly shorter than
    // its raw bounds, so use a more forgiving threshold here.
    let portraitish = safe.height > safe.width * 1.15;
    let compact = safe.width < 600.0 || portraitish;
    let margin = if compact { 12.0 } else { SHARED_HEADER_MARGIN };
    let group_gap = if compact { 8.0 } else { SHARED_HEADER_GAP };
    let cell_gap = if compact { 6.0 } else { SHARED_HEADER_CELL_GAP };
    let pill_padding = if compact {
        8.0
    } else {
        SHARED_HEADER_PILL_PADDING
    };
    let maximum_size = if compact { 44.0 } else { SHARED_HEADER_SIZE };
    let x = safe.x + margin;
    let y = safe.y + margin;
    let available_width = (safe.width - margin * 2.0).max(0.0);
    let size = maximum_size
        .min(((available_width - group_gap - cell_gap * 2.0 - pill_padding * 2.0) / 4.0).max(0.0));
    if size < 1.0 {
        return SharedHeaderGeometry {
            nodes: Vec::new(),
            custom_x: x,
            y,
            size: 0.0,
        };
    }

    let surface = [0.02, 0.04, 0.05, 0.92];
    let logo_rect = UiRect {
        x,
        y,
        width: size,
        height: size,
    };
    let controls_x = x + size + group_gap;
    let cell_size = size;
    let controls_width = cell_size * 3.0 + cell_gap * 2.0 + pill_padding * 2.0;
    let icon_size = (cell_size - 16.0).max(1.0);

    let mut nodes = vec![header_surface(
        "__shared_header_logo_surface",
        logo_rect,
        size * 0.5,
        surface,
    )];
    nodes.push(header_image(
        "__shared_header_logo",
        UiImage::Logo,
        logo_rect.inset((cell_size - icon_size) * 0.5),
        false,
    ));

    let controls_rect = UiRect {
        x: controls_x,
        y,
        width: controls_width,
        height: size,
    };
    nodes.push(header_surface(
        "__shared_header_controls_surface",
        controls_rect,
        size * 0.5,
        surface,
    ));
    for (index, image) in [UiImage::Cube, UiImage::Chat, UiImage::Voice]
        .into_iter()
        .enumerate()
    {
        let cell = UiRect {
            x: controls_x + pill_padding + index as f32 * (cell_size + cell_gap),
            y,
            width: cell_size,
            height: size,
        };
        nodes.push(header_image(
            match image {
                UiImage::Cube => "__shared_header_cube",
                UiImage::Chat => "__shared_header_chat",
                UiImage::Voice => "__shared_header_voice",
                UiImage::Logo => unreachable!(),
            },
            image,
            cell.inset((cell_size - icon_size) * 0.5),
            true,
        ));
    }
    SharedHeaderGeometry {
        nodes,
        custom_x: controls_rect.x + controls_rect.width + group_gap,
        y,
        size,
    }
}

struct SharedModalGeometry {
    nodes: Vec<UiRenderNode>,
    hit_regions: Vec<UiHitRegion>,
}

fn shared_modal_nodes(
    viewport: UiViewport,
    safe: UiRect,
    progress: f32,
    target: f32,
    selected_tab: usize,
) -> SharedModalGeometry {
    if viewport.width <= 0.0
        || viewport.height <= 0.0
        || (progress <= 0.0 && target <= 0.0)
    {
        return SharedModalGeometry {
            nodes: Vec::new(),
            hit_regions: Vec::new(),
        };
    }

    let compact = safe.width < 600.0 || safe.height > safe.width * 1.15;
    let horizontal_margin = if compact { 12.0 } else { 18.0 };
    let top_gap = if compact { 48.0 } else { 52.0 };
    let panel = UiRect {
        x: (safe.x + horizontal_margin).min(viewport.width),
        y: (safe.y + top_gap).min(viewport.height),
        width: (safe.width - horizontal_margin * 2.0).max(0.0),
        height: (safe.height - top_gap).max(0.0),
    };
    let eased = progress * progress * (3.0 - 2.0 * progress);
    let slide_distance = (viewport.height - panel.y).max(0.0);
    let animated_panel = UiRect {
        y: panel.y + slide_distance * (1.0 - eased),
        ..panel
    };
    let scrim = UiRect {
        x: 0.0,
        y: 0.0,
        width: viewport.width,
        height: viewport.height,
    };
    let mut nodes = vec![modal_node(
        "__shared_modal_scrim",
        UiNodeKind::Modal,
        scrim,
        Some([0.01, 0.015, 0.02, 0.54 * progress]),
        None,
        0.0,
    )];
    let mut hit_regions = vec![UiHitRegion {
        id: "__shared_modal_scrim".to_owned(),
        action: "shared.modal.close".to_owned(),
        kind: UiNodeKind::Modal,
        rect: scrim,
        disabled: false,
    }];

    nodes.push(modal_node(
        "__shared_modal_panel",
        UiNodeKind::Panel,
        animated_panel,
        Some([0.10, 0.12, 0.15, 0.94]),
        Some([0.52, 0.60, 0.64, 0.32]),
        if compact { 18.0 } else { 22.0 },
    ));
    hit_regions.push(UiHitRegion {
        id: "__shared_modal_panel".to_owned(),
        action: String::new(),
        kind: UiNodeKind::Panel,
        rect: animated_panel,
        disabled: false,
    });

    let tab_padding = if compact { 12.0 } else { 18.0 };
    let tab_gap = if compact { 5.0 } else { 8.0 };
    let tab_row = UiRect {
        x: animated_panel.x + tab_padding,
        y: animated_panel.y + tab_padding,
        width: (animated_panel.width - tab_padding * 2.0).max(0.0),
        height: if compact { 46.0 } else { 50.0 },
    };
    let tab_width = ((tab_row.width - tab_gap * 4.0) / 5.0).max(0.0);
    for (index, label) in ["Home", "Settings", "People", "Report", "Help"]
        .into_iter()
        .enumerate()
    {
        let tab_rect = UiRect {
            x: tab_row.x + index as f32 * (tab_width + tab_gap),
            y: tab_row.y,
            width: tab_width,
            height: tab_row.height,
        };
        let selected = index == selected_tab;
        let id = format!("__shared_modal_tab_{index}");
        nodes.push(modal_node(
            &id,
            UiNodeKind::Button,
            tab_rect,
            Some(if selected {
                [0.24, 0.30, 0.34, 0.98]
            } else {
                [0.13, 0.16, 0.19, 0.76]
            }),
            Some(if selected {
                [0.78, 0.87, 0.88, 0.72]
            } else {
                [0.50, 0.59, 0.62, 0.24]
            }),
            if compact { 12.0 } else { 14.0 },
        ));
        if let Some(node) = nodes.last_mut() {
            node.text = label.to_owned();
            node.font_size = if compact { 12.0 } else { 14.0 };
            node.text_align = UiAlignment::Center;
        }
        hit_regions.push(UiHitRegion {
            id,
            action: "shared.modal.tab".to_owned(),
            kind: UiNodeKind::Button,
            rect: tab_rect,
            disabled: false,
        });
    }

    // The body intentionally has no copy or controls from the reference
    // image. It is simply a quiet surface reserved for future tab content.
    let body = UiRect {
        x: animated_panel.x + tab_padding,
        y: tab_row.y + tab_row.height + tab_padding,
        width: (animated_panel.width - tab_padding * 2.0).max(0.0),
        height: (animated_panel.height - tab_padding * 3.0 - tab_row.height).max(0.0),
    };
    nodes.push(modal_node(
        "__shared_modal_body",
        UiNodeKind::Panel,
        body,
        Some([0.07, 0.09, 0.11, 0.52]),
        Some([0.42, 0.50, 0.54, 0.18]),
        if compact { 12.0 } else { 16.0 },
    ));
    hit_regions.push(UiHitRegion {
        id: "__shared_modal_body".to_owned(),
        action: String::new(),
        kind: UiNodeKind::Panel,
        rect: body,
        disabled: false,
    });

    if selected_tab == 0 {
        let link = UiRect {
            x: body.x + tab_padding,
            y: body.y + tab_padding,
            width: body.width.min(if compact { 280.0 } else { 320.0 }),
            height: if compact { 50.0 } else { 56.0 },
        };
        let mut link_node = modal_node(
            "__shared_modal_about_link",
            UiNodeKind::Button,
            link,
            Some([0.20, 0.42, 0.55, 0.94]),
            Some([0.58, 0.80, 0.88, 0.68]),
            if compact { 14.0 } else { 16.0 },
        );
        link_node.text = "https://cubacadabra.com/about/".to_owned();
        link_node.font_size = if compact { 13.0 } else { 15.0 };
        link_node.text_align = UiAlignment::Center;
        nodes.push(link_node);
        hit_regions.push(UiHitRegion {
            id: "__shared_modal_about_link".to_owned(),
            action: "shared.about.open".to_owned(),
            kind: UiNodeKind::Button,
            rect: link,
            disabled: false,
        });
    }

    SharedModalGeometry { nodes, hit_regions }
}

fn modal_node(
    id: &str,
    kind: UiNodeKind,
    rect: UiRect,
    background: Option<[f32; 4]>,
    border_color: Option<[f32; 4]>,
    corner_radius: f32,
) -> UiRenderNode {
    UiRenderNode {
        id: id.to_owned(),
        kind,
        rect,
        text: String::new(),
        icon: None,
        background,
        foreground: [0.95, 0.97, 0.96, 1.0],
        border_color,
        border_width: if border_color.is_some() { 1.0 } else { 0.0 },
        corner_radius,
        font_size: default_font_size(),
        text_align: UiAlignment::Start,
        accent: [0.10, 0.55, 0.92, 1.0],
        image: None,
        image_invert: false,
        value: 0.0,
        value_x: 0.0,
        value_y: 0.0,
        checked: false,
        pressed: false,
        disabled: false,
    }
}

fn shared_modal_tab_index(id: &str) -> Option<usize> {
    id.strip_prefix("__shared_modal_tab_")?.parse().ok()
}

fn header_surface(
    id: &str,
    rect: UiRect,
    corner_radius: f32,
    background: [f32; 4],
) -> UiRenderNode {
    UiRenderNode {
        id: id.to_owned(),
        kind: UiNodeKind::Panel,
        rect,
        text: String::new(),
        icon: None,
        background: Some(background),
        foreground: [1.0; 4],
        border_color: None,
        border_width: 0.0,
        corner_radius,
        font_size: default_font_size(),
        text_align: UiAlignment::Start,
        accent: [0.0, 0.58, 1.0, 1.0],
        image: None,
        image_invert: false,
        value: 0.0,
        value_x: 0.0,
        value_y: 0.0,
        checked: false,
        pressed: false,
        disabled: false,
    }
}

fn header_image(id: &str, image: UiImage, rect: UiRect, image_invert: bool) -> UiRenderNode {
    UiRenderNode {
        id: id.to_owned(),
        kind: UiNodeKind::Panel,
        rect,
        text: String::new(),
        icon: None,
        background: None,
        foreground: [1.0; 4],
        border_color: None,
        border_width: 0.0,
        corner_radius: 0.0,
        font_size: default_font_size(),
        text_align: UiAlignment::Start,
        accent: [0.0, 0.58, 1.0, 1.0],
        image: Some(image),
        image_invert,
        value: 0.0,
        value_x: 0.0,
        value_y: 0.0,
        checked: false,
        pressed: false,
        disabled: false,
    }
}

