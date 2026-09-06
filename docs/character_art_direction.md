# Character art direction

Status: active direction; one person in development, visual approval open.

Cubacadabra characters are **friendly, expressive adventurers with a soft-cube
influence**. They should read as people and creatures wearing recognizable
clothes. Appeal in an ordinary paused gameplay view is the design goal.

## Shape and personality

Keep broad readable shapes, gentle angular curves, graphic faces and playful
proportions. Choose geometry for what it represents: swept hair, a relaxed
sleeve, a palm with a thumb, a sneaker with a toe and heel. Rounded cuboids are
one tool, not a required construction method or silhouette.

**“Enchanted toy,” visible assembly and “one imaginary toy company” are no
longer requirements.** An ordinary person needs connected clothing and anatomy.
Magic belongs to actions, equipment, places and optional creature designs.
A plain hoodie character must feel complete with all effects disabled.

A smooth or slightly plastic-looking head can work. Different materials should
read as different things. Texture, gloss and animation cannot repair a weak shape.

## Current focus: one casual person

Pause new species, costumes and accessories. Keep existing catalog entries
working as compatibility and rendering fixtures. Their existence does not
establish visual approval.

Develop a complete outfit: swept hair framing the face, a quiet resting
expression, dropped hoodie shoulders, visible palms and thumbs, shorts or
trousers emerging below the hem, and separated sneakers. The hood sits behind
the neck and reads from behind. Sleeves narrow toward cuffs without separate
padded beads. Give hair an irregular silhouette before adding follow-through.

Start the resting face with small dark eyes and a relaxed closed smile.
Compare no brows against thin brows under identical light. Whites, irises,
highlights, blush and a nose are optional design choices, not fidelity goals.
Keep the expression system; do not expand its preset count to solve appeal.

Motion should preserve the character: grounded stance soles, clear foot
lifting, consistent knee direction, relaxed elbows and a calm return to idle.
A wave must read as a hand gesture even without skin color.

## Proportion studies, not a locked specification

[Current engine studies](art/person/README.md) compare:

| Study | What changes |
| --- | --- |
| Everyday | Balanced head/body relationship, small dark eyes, no brows |
| Longer legs | Smaller head, longer legs, higher garment and thin brows |
| Soft shoulders | Shorter stance, wider garment, dropped shoulders, larger head and quiet eye highlights |

Everyday is the working implementation, not the approved winner. Compare the
same outfit, colors, light and camera. Review front, side, three-quarter, back,
face close-up, black silhouette and ordinary gameplay distance. A flattering
close-up or passing test cannot establish appeal.

## Acceptance

1. Standing still: would someone choose to play as this character?
2. Moving: do hair, hands, clothes and feet remain readable and feel connected?
3. Without decoration: does it work without glow, particles or fine patterns?

Show actual alternatives to intended players. Ask which they would choose and
what they like or dislike, without leading language. Record the selected
images and concrete revisions here after review. No player preference or
child appeal has been validated yet.

After one person passes, adapt the approved relationships to other species
and garments. Let broader world-art rules follow that result.

## Engineering boundaries

Retain indexed meshes, hierarchy, presentation state, instancing, LOD,
appearance IDs, persistence, camera controls and platform support.
Official meshes may use authored profiles, scripts or a bounded modeling-tool
export; a universal runtime importer is not a prerequisite.

Collision and identity remain gameplay contracts. The API name `magic`
remains for compatibility and does not prescribe glowing anatomy.
See [runtime and verification](character_runtime.md).
