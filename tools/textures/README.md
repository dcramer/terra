# Textures

Makes the game's materials (D14, D191): the tileable surfaces a model or the ground is drawn with, as `assets/materials/<name>/`, each a colour map, a normal map (OpenGL's convention, as Bevy reads it), an ARM map (occlusion, roughness and metal in red, green and blue) and, for parallax, a height map. Where every material came from is in [assets/materials/README.md](../../assets/materials/README.md), which each step keeps up to date.

```bash
node tools/textures/textures.ts fetch rough_wood                        # a Poly Haven material, CC0, at 2K
node tools/textures/textures.ts material shakes art/textures/shakes.png --tile y   # from a generated image
node tools/textures/textures.ts stack ground forest_ground_04 leafy_grass rock_face_03 sand_01
```

- **fetch** downloads a [Poly Haven](https://polyhaven.com) material by its id. Libraries come first: their maps are measured, not guessed. The material's size in metres is printed; a kit's file says it.
- **material** makes one from a single image no library has, such as a Tripo image of a roof of shakes: it crops the image to the length at which it repeats itself, so rows tile without a seam, and works out a normal map and a height from its light and dark (weaker than a scanned material's). `--strength` sets how deep the relief reads, `--roughness` its roughness.
- **stack** puts several materials' colour and normal maps one above the next, 1024 pixels each, for a surface that blends them, such as the ground, which loads them as a texture array. Each layer's occlusion is multiplied into its colour. The stacked materials' own folders are not needed after; the stack's credit names them.

It needs ImageMagick (`convert`). The tests are `node --test tools/textures/textures.test.ts`.
