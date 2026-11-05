# Quaternius's bodies and outfits (D599)

The character base the game is moving to: their shapes, our paint ([research](../../../research/2026-11-02-character-base.md)).

| Folder | Pack | Edition and version | Licence |
|---|---|---|---|
| `universal-base-characters/` | [Universal Base Characters](https://quaternius.itch.io/universal-base-characters) | Source edition; files dated to 2025-12-16 | CC0 1.0, its `License_Source.txt`, verbatim |
| `modular-outfits-fantasy/` | [Modular Character Outfits: Fantasy](https://quaternius.itch.io/modular-character-outfits-fantasy) | Source edition, v2.1; files dated to 2026-01-29 | CC0 1.0, its `License_Source.txt`, verbatim |

Bought by the developers from quaternius.itch.io on 2026-11-02. CC0 asks for no credit; we credit Quaternius anyway (`THIRD_PARTY.md`).

**What is here.** Only what v0 uses, each file copied byte for byte from the pack's glTF export folder (`Exports/Godot - UE` and `Exports/glTF (Godot-Unreal)/Modular Parts`), so the `.gltf` files find their `.bin` and textures beside them:
- the Regular man and woman, full body and head only, with their eyes, brows, hair 1 and 2 and dark skin textures;
- the Regular man's upper body (head, shoulders, bare arms and hands, cut by Quaternius from the full body), the one bare piece the packs ship besides the head, for trying a bare arm swapped in (D600);
- the Ranger and Peasant outfits for both, piece by piece (arms, body, legs, feet, hood, pauldrons), and the outfit textures at their shipped 4096²;
- the man's Knight, Knight Cloth, Noble and Wizard, piece by piece with their accessories (pauldrons, scarf, gorget, armet, horned helmet, crown), their maps, and each one's two other colourways (`T_<Outfit>_2_BaseColor.png`, `_3_`, from the pack's `Textures/<Outfit>/`);
- the men's hairstyles and beards (Balding, Buzzed, Dreads, Mohawk, Ponytail, SimpleParted, SlickBack; Beard, Moustache, MuttonChops), from `Hairstyles/Rigged to Head Bone/glTF (Godot -Unreal)/Male/`, on the full 65-joint skeleton.

`node tools/characters/wardrobe.ts`'s first stages hold them: every file against `SHA256SUMS` and the private repository's own manifest, and no script that reshapes meshes may be given one (D599, `tools/characters/theirs.py`).

The glTF exports, not the `.blend` sources: the exports are the pieces as Quaternius finished them (the mirror and geometry-node modifiers in the `.blend` applied), so they are the reference every output is checked against. The `.blend` files, the other bodies, outfits and hairstyles stay in the zips (`Universal Base Characters[Source].zip`, `Modular Character Outfits - Fantasy[Source].zip`).

**Untouched.** Nothing here is edited, cut, resized or re-exported (the developers, 2026-11-02: their meshes, UVs, normals, weights, seams, split and bones stay as made). Our tools write separate outputs built from these. `SHA256SUMS` holds every file's hash; check from the repository root with `sha256sum -c art/characters/quaternius/SHA256SUMS`. That our outputs keep their pieces vertex for vertex and weight for weight is checked by `node tools/characters/intact.ts OUTPUT.glb` (D599), run on every output built from these.

**Not here.** Their Bestiary: Dungeon Monsters Kit is under the Quaternius Asset License v1.0, not CC0: it lets us use the monsters in the game but not pass the files on, and a public repository passes them on. None of it is committed.
