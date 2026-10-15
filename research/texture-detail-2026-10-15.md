# Texture detail of the construction materials, near

2026-10-15. Why the logs' and the stone's textures read soft a metre or two off, measured against known-good references, and what was done about it (D362).

The developers: "the log textures still look pretty bad. stone might need more improved details too? its important that the detail on our textures is great." The logs had been reworked several times (D262, D290), each time on the look of the bark. This note steps back and measures the whole path from source scan to pixel first.

## How it was measured

- **Stills**: the kit sheet's preview of each material (`--kit-sheet --sheet-rows "<material>: preview"`, D349), a clean stretch of the house's front looked at square on from 1, 2 and 5 m (`--vantage`), noon, offscreen 1920x1080, the Windows build on the real GPU, high and low presets.
- **Screen density**: at 1080p and the game's 55° vertical field of view, a surface square on is 1,037 / d pixels a metre at d metres: 1,037 at 1 m, 519 at 2 m, 207 at 5 m. The industry's target for first-person close-ups is 512 to 1,024 texels a metre.
- **Sharpness**: a picture's *finest octave*, the RMS lost by halving it and doubling it again (box down, triangle up), as a share of its RMS contrast. A sharp scan loses a third or more; an upscaled or blurred picture almost nothing. Measured on the source maps, and on a crop of each still.
- **TAA**: the same frame with Bevy's temporal anti-aliasing taken off the camera live over the remote protocol (`world.remove_components`), then with contrast-adaptive sharpening at 0.4 to 1.0 put on.

## What was found, by how much it cost

1. **The arrays halved every scan.** Every texture set was a 1,024-texel layer (`arrays::SIDE`), and the pack tool cut every map to 1,024 too, so the 2K Poly Haven scans lost three quarters of their texels. A stone wall copy is 2 m: 512 texels a metre, two pixels a texel at 1 m. Slate 427 (2.4 pixels a texel), split shakes 320 by 640 (3.2), the bark 1,090 round and 512 along. Only hewn timber (rough wood at half a metre, 2,048) met the target. Stone at 1 m: finest octave 8.5% of the crop's contrast.
2. **TAA took more than half of what was left.** On a log a metre off, the finest octave is 20.5% of the crop without TAA and 8.7% with it: TAA's blend of jittered frames is a filter about a pixel wide, and nothing sharpened after it at full size (only the low preset's upscaled picture was sharpened). Its mip bias was already -1 (`MipBias`'s default) and its history is read Catmull-Rom; the softening is inherent. Contrast-adaptive sharpening gives back 9.6% at 0.4, 10.7% at 0.6, 15.1% at 1.0.
3. **The log's bark bake.** `pine_log` was baked from the Tripo log at 1,024, a metre round by 2 m along: half the Tripo texture's own 4K density along the log. Its source is soft, a generated texture: its colour keeps 13% in the finest octave against a Poly Haven bark scan's 37%, its normal map 14% against 47%, and its normal's fine relief is a sixth of the scan's (RMS 0.006 against 0.037). The bake stretched it along by 1.25 by taking the *nearest* row, so every fifth row was doubled, a stair in the grain. And its colour kept the light Tripo painted it in: by sixteenths round the log, from 24 to 146 of 255, six times darker underneath than on top, which drew a near-black band under every log on top of the game's own shading.
4. **BC1 colour.** The colour was BC1 from a real-time encoder (the box's corners, inset): four shades a 4x4 block in 5:6:5. On 400 made blocks of two tones and grain, BC7 mode 6 (sixteen shades at 7 bits) has under half BC1's squared error (`bc::tests`). BC7 for colour and BC5 for normals is the standard; normals were already BC5.
5. **Mipmaps** were averaged in sRGB, which darkens each level, and normal maps' levels were not made unit again, which flattens them with distance. Small next to the rest.
6. **The quoins** (D215 as amended today) are cut from the wall's own largest stones, each blown up to a quoin's face: two to three pixels a texel at 1 m before the detail maps, blotchy and soft. The detail maps carry them now; their own size is D215's to judge.

Sampling was not at fault: 16x anisotropic filtering on every array (one sampler, set on the colour array), repeat, and a -1 bias.

## What shipped games do, and what was done

- **Texel density by texture quality**: colour and normal maps 2,048 a side at high and medium, 1,024 at low (`Quality::texture_side`), as a texture quality setting keeps or drops a top mip; occlusion-roughness-metal and height stay 1,024, since they carry no fine detail. A stone wall is now 1,024 texels a metre, a texel a pixel at 1 m.
- **BC7 colour** (mode 6, its line along the block's principal axis then fitted to the steps chosen by least squares, after Rich Geldreich's bc7enc), BC5 normal, BC1 ARM, BC4 height, encoded on a worker as a set is made resident; mipmaps in linear light, normals renormalised.
- **Detail maps**, the standard way to crisp close detail without huge textures (Unity's detail albedo and normal, Unreal's detail texturing): each pattern lays a shared high-frequency texture of its material's grain over its textures near, wood's (from Poly Haven's rough wood) every 40 cm along the grain, stone's (from Poly Haven's rock surface) every 50 cm, faded out from 3 to 8 m, not drawn at the low preset. Data per pattern (`details`), a pack able to name its covering's or take one away.
- **Sharpening after TAA** at every preset, 0.6 at high.
- **The bark rebaked** at 2,048 from the Tripo log's 4K maps, read between rows, its painted light taken out round it (`bake.py`'s `delit`, a fifth of it kept): now 67 to 99 of 255 round the log.

## Results

To follow in this note: stills, texel density, memory and frame time before and after.
