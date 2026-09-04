# Shared in-game UI runtime

The engine owns a retained two-dimensional UI document that is laid out in
logical viewport units, hit-tested by Rust, and drawn as an orthographic pass
after the 3D world. This is the portable layer for experience HUDs, touch
controls, and in-game modals. Platform shells still own operating-system UI
such as sign-in, purchases, permissions, and text input.

Every game also receives an engine-owned top-left header containing the shared
logo, cube, chat, and voice controls. These controls are rendered from the
bundled image assets, are not part of the Luau document, and currently consume
their tap area without emitting UI events. Games should define only the
game-specific controls that follow this shared header.

## Luau API

An experience can install a document during `on_start`. `set_document` accepts
either a Luau table or a JSON string. Tables are preferred for authored game
code.

```lua
local game = {}

function game.on_start(api)
    api.ui:set_document({
        nodes = {
            {
                id = "build-dock",
                kind = "panel",
                layout = {
                    anchor = "bottom",
                    width = "90%",
                    maxWidth = 560,
                    height = 60,
                    offset = { 0, -16 },
                    padding = 8,
                    direction = "row",
                    align = "center",
                    gap = 6,
                },
                style = {
                    background = "#101820E8",
                    cornerRadius = 30,
                },
                children = {
                    {
                        id = "tool",
                        kind = "button",
                        text = "PLACE",
                        action = "build.tool",
                        layout = { width = 84, height = 44 },
                        style = {
                            background = "#FFFFFF1F",
                            cornerRadius = 22,
                            textAlign = "center",
                        },
                    },
                    {
                        id = "shape",
                        kind = "button",
                        text = "CUBE",
                        action = "build.shape",
                        layout = { width = 76, height = 44 },
                        style = {
                            background = "#FFFFFF1F",
                            cornerRadius = 22,
                            textAlign = "center",
                        },
                    },
                    {
                        id = "use-tool",
                        kind = "button",
                        text = "BUILD",
                        action = "build.use",
                        layout = { width = "fill", height = 44 },
                        style = {
                            background = "#0094FFFF",
                            cornerRadius = 22,
                            textAlign = "center",
                        },
                    },
                },
            },
        },
    })
end

function game.on_ui_event(api, event)
    if event.action == "build.shape" then
        api.ui:set_text("shape", "BEAM")
    end
end

return game
```

The mutation methods are:

- `api.ui:set_document(document)`
- `api.ui:clear()`
- `api.ui:set_text(id, text)`
- `api.ui:set_value(id, number)`
- `api.ui:set_checked(id, boolean)`
- `api.ui:set_visible(id, boolean)`

Interactive nodes emit `on_ui_event(api, event)`. Every event has `node_id`,
`action`, and `phase`; sliders and toggles also include `value`. Events are
duplicated to the host-facing queue, so Luau can update presentation while the
platform adapter forwards commands that require networking or an OS service.

## Document model

Supported node kinds are `panel`, `stack`, `text`, `button`, `toggle`,
`slider`, and `joystick`. Nodes are nested through `children` and each ID must
be unique. Joystick events contain normalized `x` and `y` values and emit a
zero vector on release or cancellation, allowing the shared UI to own mobile
movement controls safely with multiple simultaneous pointers.

Root nodes are positioned inside the safe viewport using `layout.anchor`:
`topLeft`, `top`, `topRight`, `left`, `center`, `right`, `bottomLeft`, `bottom`,
or `bottomRight`. Child nodes flow in a `row` or `column` according to their
parent. Width and height accept logical points, `auto`, `fill`, or a percentage
such as `90%`. `maxWidth` and `maxHeight` keep UI from stretching on tablets
and desktop displays.

Root layout respects the platform safe area by default. A full-screen modal
scrim can set `layout.ignoreSafeArea = true`; setting `blocksInput = true` on
that panel consumes pointer input without emitting an action, preventing taps
from also moving the camera. Root nodes later in the document are visually and
interactively above earlier roots, so the modal panel can follow its scrim.

Containers support `padding`, `gap`, `align`, and `justify`. Alignments are
`start`, `center`, `end`, and `stretch`; justification additionally supports
`spaceBetween`. Every interactive node gets at least a 44-point hit target
unless constrained by its available parent space; a joystick defaults to 120
points square.

Colors use `#RRGGBB` or `#RRGGBBAA`. Styles currently support `background`,
`foreground`, `borderColor`, `borderWidth`, `cornerRadius`, `fontSize`,
`textAlign`, and `accent`.

Documents are limited to 512 nodes and 32 levels of nesting. The current first
Luau-authored UI intentionally omits image assets, clipping/scrolling, rich font shaping,
keyboard focus, and the native accessibility mirror; those can be added
without changing document ownership or input routing.

## Platform ABI

Before rendering or sending pointer events, the platform provides logical
viewport dimensions, display scale, and safe-area insets:

```c
engine_set_ui_viewport(engine, width, height, scale,
                       safe_top, safe_right, safe_bottom, safe_left);
```

Pointer coordinates use the same logical coordinate space. Pointer phase is
`0` for down, `1` for move, `2` for up, and `3` for cancel. A nonzero return
value means the UI consumed the event, so the platform must not also treat it
as camera input.

```c
uint8_t consumed = engine_ui_pointer(engine, pointer_id, phase, x, y);
```

Hosts can install JSON documents through `engine_ui_document_buffer_ptr` and
`engine_load_ui_document_buffer`. They read actions by polling
`engine_ui_poll_event`; the current event remains available as UTF-8 JSON from
`engine_ui_event_ptr` and `engine_ui_event_len` until the next poll.
