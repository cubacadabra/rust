# Built-in terrain materials

These are engine-owned 512×512 RGBA albedo tiles. The runtime decodes them into
a seven-layer texture array, generates mip levels, and samples them with
world-space triplanar mapping. Game packages refer to semantic terrain IDs and
do not include copies of these images.

| Terrain ID | Array layer | Surface recipe |
| --- | ---: | --- |
| `grass` | 0 / 1 | Green top, exposed soil sides; ground on downward-facing surfaces |
| `ground` | 2 | Compacted warm earth on every orientation |
| `rock` | 3 | Cool gray stone |
| `sand` | 4 | Warm sand |
| `mud` | 5 | Dark earth |
| `snow` | 6 | Pale cool snow |

The terrain renderer uses a world-space frequency of 0.18 tile cycles per world
unit. When terrain art is disabled, the existing procedural material colors
remain available; terrain storage, meshing, and collision do not depend on
these image files.

The artwork was generated as a coordinated stylized set, reduced from the
original square renders to 512×512, and reviewed as 3×3 repeated tiles. The
source prompts requested seamless edges and neutral albedo without baked
lighting. Re-check tiling and in-world scale when changing these assets.
