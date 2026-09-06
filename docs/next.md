The next leap should be driven by visible personality, silhouette, and play feedback. The renderer already has the technical groundwork: rounded geometry, rigs, expressions, outfits, effects, LOD, instancing, and MSAA. The current captures show that the result still reads as one shared humanoid body with costume variations.

My top ten, in order:

1. Redesign the three hero silhouettes

Make the person, cat, and dragon unmistakable in solid black. Give each a different head profile, torso mass, stance, hand shape, feet, and appendage rhythm. The current species are still too close in overall construction.

Done when each silhouette is identifiable from a thumbnail and from the back.

2. Turn the face into the emotional center

Enlarge the eyes slightly, give each species its own eye and mouth proportions, and add stronger eyebrow, eyelid, cheek, and mouth shapes. Connect expressions to gameplay: surprise on landing, concentration while sprinting, delight during collectibles, sleepy idle, and excitement near friends.

The current face system is a good base, but the expressions need to be visible during ordinary gameplay, not only close inspection.

3. Make the magic seams a signature behavior

The seam glow should occasionally pulse, breathe, and emit a tiny spark when the character jumps, lands, waves, gets excited, or equips an item. Keep the normal idle seam quiet and elegant.

The important change is behavior: the viewer should understand that the pieces are magically held together.

4. Make outfits change the silhouette

Several current outfits add trim and surface details while preserving nearly the same body shape. Author real volumes: an oversized hoodie hem, a wide puffer, a flared raincoat, a huge wizard hat, large knight boots, and floppy pajama cuffs.

The six outfits should look different as flat color blobs before material detail is added.

5. Build a signature cartoony animation language

Replace the mostly sinusoidal limb motion with authored motion beats:
- squash before jumping
- explosive extension
- dangling limbs in the air
- exaggerated landing compression
- head leading turns
- torso lean and foot recoil while running
- big readable waves and reactions

Kids will feel this improvement immediately. Motion is likely a larger “wow” multiplier than additional geometry.

6. Give materials stronger toy-like contrast

Establish a small, unmistakable material library: soft plastic skin, matte cloth, fuzzy cloth, rubber, glossy rainwear, painted metal, and magical emission.

The current shader has useful roughness and weave cues, but most surfaces still read as similarly shaded colored geometry. Materials need different highlight shapes, edge response, and value separation.

7. Add real close-up garment detail

Move the catalog beyond procedural colored parts. Add a few small filtered textures or masks for cloth weave, denim variation, quilting, stitched motifs, embroidered stars, rubber tread, and raised seams.

The current catalog explicitly has zero texture payload, so this is a clear opportunity for the “cute at a distance, beautiful up close” identity.

8. Add asymmetry and collectible personality

Let characters equip small personal details: mismatched socks, one ear accessory, stickers, freckles, patches, tiny backpacks, glasses, pins, charms, or a favorite magical color.

Two characters wearing the same hoodie should still feel like different kids. Asymmetry is cheap and highly memorable.

9. Create a character playground for rapid visual review

Make one deterministic in-game showcase where the three bodies and six outfits can:
- rotate on a small stage
- run, jump, land, wave, emote, and idle
- switch expressions
- trigger seam effects
- show front, side, back, and silhouette views

This should be the daily art-review tool, not just a capture utility. The existing capture work in docs/ magic_characters_polish.md:89 is a strong foundation.

10. Make the world speak the same visual language

Apply the character rules to a few highly visible objects: one squircle tree, one toy vehicle, one collectible, and one small building detail. Characters will feel much more native to Cubacadabra when the world shares their softened edges, material language, and magical accents.

The existing soft-cubism examples (assets/characters/soft_cubism_examples.json) are a good starting contract, but they need to become visible authored assets.
