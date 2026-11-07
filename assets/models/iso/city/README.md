# Iso city and wasteland props

Scanned and photo-textured street props for the iso experiment's neon street at night in rain and its post-apocalyptic wasteland (iso-kit, D634). Each is Poly Haven's 1k glTF packed as it came into one GLB, +Y up, metres, 1024 px textures (colour, ARM, OpenGL normal). All are CC0 ([Poly Haven's licence](https://polyhaven.com/license)). The prop budget is 8k triangles (iso-art's asset.md, D628): those over it were cut to it by `tools/blender/decimate.py` on the render worker (collapse, UVs and textures kept, written whole; iso-kit j29), their Triangles reading "was" the source's count.

| File | Source | Author | Triangles | Size (m, x y z) | Note |
|---|---|---|---|---|---|
| `aircon-unit.glb` | [Exterior Aircon Unit](https://polyhaven.com/a/exterior_aircon_unit) | Monsta3D | 7,920 (was 18,986) | 1.80 x 0.93 x 0.37 | its foot 0.32 m below the origin (wall bracket); 7.1 MB |
| `barrel-stove.glb` | [Barrel Stove](https://polyhaven.com/a/barrel_stove) | MP | 7,919 (was 9,216) | 0.60 x 0.86 x 0.60 | a burning drum for the wasteland or an alley |
| `cardboard-box.glb` | [Cardboard Box 01](https://polyhaven.com/a/cardboard_box_01) | Rahul Chaudhary | 7,919 (was 16,952) | 0.39 x 0.34 x 0.52 | |
| `covered-car.glb` | [Covered Car](https://polyhaven.com/a/covered_car) | MP | 7,914 (was 12,592) | 1.78 x 1.41 x 4.34 | a car under a tarp |
| `generator.glb` | [Portable Generator](https://polyhaven.com/a/portable_generator) | James Ray Cock | 7,915 (was 26,419) | 0.82 x 0.58 x 0.56 | |
| `jerrycan.glb` | [Metal Jerrycan](https://polyhaven.com/a/metal_jerrycan) | Sean Buckley | 7,920 (was 20,022) | 0.35 x 0.46 x 0.17 | |
| `manhole-cover.glb` | [Water Manhole Cover](https://polyhaven.com/a/water_manhole_cover) | Raunox | 6,301 | 0.69 x 0.07 x 0.69 | |
| `military-crate.glb` | [Old Military Crate](https://polyhaven.com/a/old_military_crate) | Jack Mava | 7,910 (was 20,952) | 1.82 x 0.30 x 0.98 | one mesh drawn twice, cut once and shared |
| `oil-barrel.glb` | [Barrel 01](https://polyhaven.com/a/Barrel_01) | Jorge Camacho | 2,682 | 0.56 x 0.88 x 0.56 | |
| `old-tyre.glb` | [Old Tyre](https://polyhaven.com/a/old_tyre) | MP | 2,880 | 0.60 x 0.60 x 0.16 | |
| `payphone.glb` | [Korean Public Payphone 01](https://polyhaven.com/a/korean_public_payphone_01) | UM JOORIN | 7,919 (was 48,132) | 0.40 x 0.74 x 0.30 | wall-hung; the deepest cut, a sixth of the source |
| `propane-tank.glb` | [Propane Tank](https://polyhaven.com/a/propane_tank) | Slinc | 5,238 | 0.34 x 0.55 x 0.34 | |
| `road-barrier.glb` | [Concrete Road Barrier 02](https://polyhaven.com/a/concrete_road_barrier_02) | Amal Kumar | 7,920 (was 23,822) | 1.57 x 1.11 x 0.44 | |
| `rollershutter-door.glb` | [Rollershutter Door](https://polyhaven.com/a/rollershutter_door) | MP | 1,104 | 3.08 x 2.40 x 0.30 | a shopfront's shutter |
| `security-camera.glb` | [Security Camera 01](https://polyhaven.com/a/security_camera_01) | Alexander Otterbeck, Yann Kervran | 7,920 (was 13,571) | 0.17 x 0.29 x 0.55 | rigged in the source; the GLB carries no skin |
| `security-light.glb` | [Security Light](https://polyhaven.com/a/security_light) | Maximilian Schuster | 4,668 | 0.32 x 0.53 x 0.42 | wall lamp |
| `street-lamp.glb` | [Street Lamp 01](https://polyhaven.com/a/street_lamp_01) | Josh Dean | 7,919 (was 30,610) | 0.70 x 3.87 x 0.39 | |
| `trash-bag.glb` | [Trashbag](https://polyhaven.com/a/trashbag) | Benny Weimer | 4,482 | 0.53 x 0.57 x 0.46 | |
| `trash-can.glb` | [Metal Trash Can](https://polyhaven.com/a/metal_trash_can) | GurJas Studios | 7,909 (was 13,960) | 1.77 x 0.91 x 0.55 | two bins side by side, one clean and one rusted (the source's scene); 4.9 MB |
| `utility-box.glb` | [Utility Box 02](https://polyhaven.com/a/utility_box_02) | James Ray Cock | 6,268 | 0.92 x 1.12 x 0.43 | |
| `wheel-rim.glb` | [Rusted Wheel Rim 01](https://polyhaven.com/a/rusted_wheel_rim_01) | John Hutcheson | 7,920 (was 16,440) | 0.40 x 0.40 x 0.15 | |

Building fronts: `facade-apartments.glb`, from [Modular Urban Apartments Facade](https://polyhaven.com/a/modular_urban_apartments_facade) by James Ray Cock (CC0), its 2K glTF's parts gathered into one-storey bays by `tools/blender/kit.py` from [art/iso/city/facade-apartments.json](../../../../art/iso/city/facade-apartments.json) (iso-kit j25): ten modules, each a scene of the file in this order, each facing +Z with its foot on y=0 and its middle over the origin, collapse-cut to 3,000 triangles (the flat plaster's; scenery's cap is 20k), the textures shared, 39 MB. 0 `door-large` (double panel door), 1 `door-small`, 2 `shop-window` (two sash windows to the ground), 3 `plain`, 4 `window-large`, 5 `window-double`, 6 `window-small`, 7 `window-offset` (each 3 x 3 m, a string course at its foot), 8 `crown` (the parapet, 3 x 0.75 m), 9 `pier` (0.4 x 3 m). The kit has no glazed shopfront: its doors are panels. Kenney's City Kit Commercial (CC0) downloads only through its page's script, and its flat-coloured low-poly kit would read toy-like against these scans (D634).

## Signs (`signs/`, iso-kit j11)

Neon sign faces for the kit's street (kit/street.rs `signs`), made here from text with ffmpeg's drawtext (a coloured tube with a pale core, a 14 px glow, black ground: the emissive texture): `ramen.png` (ラーメン, 256x896, a blade), `bar.png` (BAR, 768x288), `drug.png` (薬局, pharmacy, 256x512), `24h.png` (24H, 640x288). Fonts: Noto Sans CJK Medium and Liberation Sans Bold (both SIL OFL 1.1); no trademarks. Recipe: `color=c=black:s=WxH,format=rgb24`, drawtext with `borderw=7` in the tube's colour, drawtext again in the pale core, `split`, `gblur=sigma=14`, `blend=all_mode=screen` (in rgb24: in a YUV or planar format the blend tints the ground).
