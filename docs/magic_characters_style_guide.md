# Cubacadabra Soft Cubism style guide

Status: official Phase 8 starter-world guide, version `soft-cubism.v1`.

Soft Cubism is the shared visual language for official characters and the
small set of starter-world assets that accompany them. It describes decisions
that can be validated in data; it is not permission to round every collision
block or to replace the world renderer.

## Shape contract

Use one readable mass per part, broad planar faces, rounded edges, intentional
clearances, and a slightly exaggerated toy proportion. The default character
height is 3.2 engine units from sole to head, excluding hats, ears and horns.
The common rig has 15 joints and keeps gameplay collision at the existing
engine dimensions.

| Part | Official proportion |
| --- | --- |
| Head | 0.86 high, wider than tall, shallower than wide |
| Torso | about 1.02 high, visibly shorter and tapered |
| Hands | separate rounded mittens, larger than the wrist |
| Legs | two chunky articulated segments with readable stance |
| Feet | 0.30 high, broad toe, distinct sole, 0.84 forward projection |
| Fillet | 12–20% of the shortest full dimension, clamped below half that dimension |
| Rest gaps | 0.025–0.045 engine units at neck, shoulders, wrists, waist, knees and ankles |
| Animated gap | authored travel only; never enough to read as detachment |

The forward direction is local `-Z`. Feet, face features, tails, hats and
back pieces follow named anchors. Do not animate mesh dimensions for ordinary
locomotion; animate the parent transforms. Every recipe needs finite bounds,
explicit LODs, compatible body fits, and source/license provenance.

## Material and face contract

The base family is toy skin/plastic, cloth, denim, rubber, waterproof gloss,
soft metal, fuzz and restrained emission. Use diffuse plus a quiet directional
light and rim. Geometry owns seams, pockets, soles and silhouette folds;
filtered material detail owns weave, stitches and fuzz cues. A brighter diffuse
color is not a metallic reflection.

Faces use slightly vertical rounded eyes, one restrained highlight, clear brows,
and a graphic mouth. Keep the eyes and mouth readable in a black-silhouette and
ordinary gameplay review. Emotion comes from authored face parameters, not from
skin/fur color.

## Outfit and fit contract

Resolve pieces in this order: base body, underlayer, main garment, outerwear,
accessories. Occupied slots and coverage masks are explicit. A covered body
region is hidden; two pieces are never allowed to occupy the same surface just
because both are equipped. Whole-body dresses, boots, hoods, tails and wings
declare their conflicts or authored openings.

The common hoodie and raincoat are required to fit person, cat and dragon.
Other outfit/body combinations must be declared supported or fall back
atomically to the default body and hoodie. There is no automatic cloth fitting
or runtime mesh importer in the starter set.

The six official silhouettes are the everyday hoodie, puffer explorer, glossy
raincoat, star wizard, toy knight and fuzzy pajamas. They are reusable outfit
recipes, not six palette swaps and not six new character classes.

## Transfer examples

[`assets/characters/soft_cubism_examples.json`](../assets/characters/soft_cubism_examples.json)
contains the official non-character proof set:

- `cuba:squircle-tree.v1`: clustered rounded foliage over a readable trunk and ground contact;
- `cuba:rounded-cart-panel.v1`: a toy prop panel with a broad face, soft-metal trim and rubber anchors;
- `cuba:star-badge.v1`: a small accessory with a controlled emission accent and named attachment anchors.

These examples deliberately share the character rules: restrained radii,
finite bounds, authored LODs, material families, anchors, and provenance. A
new official tree, prop or accessory should first pass these metadata checks
and a front/side/three-quarter/black-silhouette review before it is added to a
world package.

## Review checklist

Review at 390×844, 768×1024, 1280×800 and 1440×900, including the actual
portrait letterboxed world viewport. Check front, side, back and three-quarter
views; close-up material and face readability; first- and third-person camera;
raised supports; idle, walk, run, jump, fall, land and turn. Keep the magic
renderer reversible through the documented legacy/magic boundary while a host
rollout is staged. The visual switch must never change simulation, collision,
identity persistence, package parsing or the eight-float snapshot.
