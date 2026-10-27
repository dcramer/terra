// Makes the game's materials (D14, docs/art-style.md): fetches a scanned one from a free
// library, makes one from a generated image no library has, and stacks several into one
// texture array for a surface that blends them, such as the ground.
//
//   node tools/textures/textures.ts fetch <polyhaven id | ambientcg:Id> [--name n] [--size 1k|2k|4k]
//   node tools/textures/textures.ts material <name> <image> [--tile y|x|xy] [--size px]
//                                            [--strength s] [--roughness r]
//   node tools/textures/textures.ts stack <name> <material…> [--size px] [--occlusion k] [--height]
//   node tools/textures/textures.ts cards <name> <card>=<image | recipe.leaves.json | keep>… [--size px] [--columns n]
//   node tools/textures/textures.ts blend <name> <base>=<metres> <over>=<metres> [--around m]
//                                         [--along m] [--cover f] [--seed n] [--saturation f] [--lighten f]
//                                         [--exposed] [--soft f] [--wood-roughness r]
//   node tools/textures/textures.ts ends <name> <ambientCG id…> [--size px] [--weather f] [--checks n]
//   node tools/textures/textures.ts detail <name> <material> [--size px]
//
// Each writes assets/materials/<name>/, and adds or updates its line in that folder's
// README.md, which says where every material came from.
//
// **fetch** downloads a Poly Haven material (CC0) as color.jpg, normal.jpg (OpenGL's
// convention, as Bevy reads it), arm.jpg (occlusion, roughness and metal) and, where it
// has one, height.jpg for parallax. `ambientcg:<Id>` fetches one from ambientCG (also CC0)
// instead, whose maps come apart in a zip: its ARM map is put together from its occlusion
// and roughness, with no metal.
//
// **stack** puts the color and normal maps of several materials one above the next, each
// --size (1024) square, for Bevy to load as an array (`ImageArrayLayout::RowCount`). Its
// occlusion is multiplied into the colour, by --occlusion (0.6), since a stack has no
// ARM map of its own.
//
// **cards** lays leaves, fronds and flowers in one atlas, assets/materials/<name>/cards.png,
// a grid of --columns (4) cells across --size (2048), with its normal map, normal.png (its
// alpha each pixel's roughness), and
// writes the table the game reads, assets/data/<name>.ron: each card's cell and the octagon
// round its leaves, so a card is cut to them. Each card is a picture drawn on a flat blue
// background, cut out of it; a recipe, `<card>.leaves.json`, composed from scanned leaves
// (leaves.ts); or `keep`, its cell as the atlas has it.
//
// **material** writes color.jpg, normal.jpg, arm.jpg and height.jpg for parallax:
//
// - **Tiling**: along each axis given, the image is cropped to the length at which it best
//   repeats itself, found by matching a band at its start against every band further on, so
//   a texture of rows (shakes, courses of stone, planks) tiles without a seam. It is then
//   scaled to --size (2048) across, keeping its shape; its height in metres is its width's
//   times its height over its width, which the material's entry says.
// - **Normal map**: the relief the light shades, worked out from how light or dark each
//   part is, as a height, by its slopes; --strength (2) sets how deep it reads. Weaker than
//   a scanned material's measured relief, which is why the libraries come first.
// - **ARM map**: occlusion, roughness and metal in red, green and blue, as Bevy reads them:
//   no occlusion, --roughness (0.85), no metal.
//
// It needs ImageMagick (`convert`).

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { compose, ROUGH, type Recipe, type Scan } from "./leaves.ts";

const ROOT = resolve(import.meta.dirname, "..", "..");
// ImageMagick 7's `magick`, or 6's `convert` where it is all there is: the render worker has
// only the first (its fetch of large_grey_tiles failed on `convert`, barrow dress round 21).
const MAGICK = (() => {
  try {
    execFileSync("magick", ["-version"], { stdio: "ignore" });
    return "magick";
  } catch {
    return "convert";
  }
})();

/** An image's lightness, a byte a pixel, and its size. */
function grey(image: string, width?: number): { data: Buffer; width: number; height: number } {
  const resize = width ? ["-resize", `${width}x`] : [];
  const size = execFileSync(MAGICK,[image, ...resize, "-format", "%w %h", "info:"]).toString().split(" ");
  const data = execFileSync(MAGICK,[image, ...resize, "-colorspace", "gray", "-depth", "8", "gray:-"], {
    maxBuffer: 1 << 28,
  });
  return { data, width: Number(size[0]), height: Number(size[1]) };
}

/** The length along an axis at which an image best repeats itself: where a band at its
 * start matches the image most closely, from half its length on. */
export function period(data: Buffer, width: number, height: number, axis: "x" | "y"): number {
  const length = axis === "y" ? height : width;
  const across = axis === "y" ? width : height;
  const band = Math.max(4, Math.round(length / 64));
  const at = (along: number, side: number) =>
    axis === "y" ? data[along * width + side] : data[side * width + along];
  let best = length;
  let bestError = Infinity;
  for (let offset = Math.round(length / 2); offset <= length - band; offset++) {
    let error = 0;
    for (let i = 0; i < band; i++) {
      for (let side = 0; side < across; side += 2) {
        error += Math.abs(at(i, side) - at(offset + i, side));
      }
    }
    if (error < bestError) {
      bestError = error;
      best = offset;
    }
  }
  return best;
}

function material(name: string, image: string, tile: string, size: number, strength: number, roughness: number) {
  const out = join(ROOT, "assets", "materials", name);
  mkdirSync(out, { recursive: true });
  // Measured on a smaller copy, then scaled up to the full image.
  const sample = grey(image, 512);
  const full = execFileSync(MAGICK,[image, "-format", "%w %h", "info:"]).toString().split(" ").map(Number);
  let [width, height] = full;
  if (tile.includes("x")) width = Math.round((period(sample.data, sample.width, sample.height, "x") * full[0]) / sample.width);
  if (tile.includes("y")) height = Math.round((period(sample.data, sample.width, sample.height, "y") * full[1]) / sample.height);
  console.log(`${name}: repeats every ${width} by ${height} of ${full[0]} by ${full[1]}`);
  const color = join(out, "color.jpg");
  execFileSync(MAGICK,[image, "-crop", `${width}x${height}+0+0`, "+repage", "-resize", `${size}x`, "-quality", "92", color]);
  // Height from lightness, softened; its slopes across and down make the normal's red and
  // green, the way up its blue (OpenGL's convention, as glTF and Bevy use).
  const slope = (kernel: string) => [
    "(", color, "-colorspace", "gray", "-blur", "0x1.5", "-define", "convolve:scale=" + strength / 8,
    "-bias", "50%", "-morphology", "Convolve", kernel, ")",
  ];
  execFileSync(MAGICK,[
    ...slope("3x3: -1,0,1 -2,0,2 -1,0,1"),
    ...slope("3x3: 1,2,1 0,0,0 -1,-2,-1"),
    "(", color, "-fill", "white", "-colorize", "100", ")",
    "-combine", "-quality", "92", join(out, "normal.jpg"),
  ]);
  // Its height, for parallax: how deep each part reads, lighter higher.
  execFileSync(MAGICK,[color, "-colorspace", "gray", "-blur", "0x2", "-normalize", "-quality", "92", join(out, "height.jpg")]);
  const green = Math.round(roughness * 255);
  execFileSync(MAGICK,["-size", "16x16", `xc:rgb(255,${green},0)`, join(out, "arm.jpg")]);
  console.log(`${name}: ${out}, ${(height / width).toFixed(3)} as tall as it is wide`);
}

/** Says where a material came from, in the materials' README, replacing what it said. */
export function credit(readme: string, name: string, line: string): string {
  const lines = readme.split("\n").filter((l) => !l.startsWith(`- \`${name}\``));
  const at = lines.findIndex((l) => l.startsWith("- `") && l > `- \`${name}\``);
  const entry = `- \`${name}\`: ${line}`;
  if (at === -1) {
    while (lines.length && lines[lines.length - 1] === "") lines.pop();
    if (!lines[lines.length - 1]?.startsWith("- `")) lines.push("");
    lines.push(entry, "");
  } else {
    lines.splice(at, 0, entry);
  }
  return lines.join("\n");
}

const README = join(ROOT, "assets", "materials", "README.md");

function readme(): string {
  return existsSync(README)
    ? readFileSync(README, "utf8")
    : "# Materials\n\nWhere each material came from. Made by `cargo xtask textures` (tools/textures).\n";
}

function record(name: string, line: string) {
  writeFileSync(README, credit(readme(), name, line));
}

/** What the README says of a material. */
function credited(name: string): string | undefined {
  const prefix = `- \`${name}\`: `;
  return readme().split("\n").find((l) => l.startsWith(prefix))?.slice(prefix.length);
}

async function download(url: string, to: string) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: ${response.status}`);
  writeFileSync(to, Buffer.from(await response.arrayBuffer()));
}

async function fetchMaterial(id: string, name: string, size: string) {
  const response = await fetch(`https://api.polyhaven.com/files/${id}`);
  if (!response.ok) throw new Error(`no Poly Haven material ${id}: ${response.status}`);
  const files = await response.json();
  const info = await (await fetch(`https://api.polyhaven.com/info/${id}`)).json();
  const out = join(ROOT, "assets", "materials", name);
  mkdirSync(out, { recursive: true });
  for (const [map, file] of [["Diffuse", "color.jpg"], ["nor_gl", "normal.jpg"], ["arm", "arm.jpg"]]) {
    const url = files[map]?.[size]?.jpg?.url;
    if (!url) throw new Error(`${id} has no ${map} at ${size}`);
    await download(url, join(out, file));
  }
  // Its height, for parallax, where it has one.
  const height = files.Displacement?.[size]?.png?.url;
  if (height) {
    await download(height, join(out, "height.png"));
    execFileSync(MAGICK,[join(out, "height.png"), "-colorspace", "gray", "-depth", "8", "-quality", "92", join(out, "height.jpg")]);
    execFileSync("rm", [join(out, "height.png")]);
  }
  const metres = info.dimensions ? ` ${(info.dimensions[0] / 1000).toFixed(2)} m across.` : "";
  record(name, `Poly Haven [${info.name ?? id}](https://polyhaven.com/a/${id}), CC0, ${size}.${metres}`);
  console.log(`${name}: ${out}${metres}`);
}

/** An ambientCG material's zip, by its id and size, as ambientCG names it: `Ground104_2K-JPG`. */
export function ambientCgFile(id: string, size: string): string {
  return `${id}_${size.toUpperCase()}-JPG`;
}

async function fetchAmbientCg(id: string, name: string, size: string) {
  const file = ambientCgFile(id, size);
  // Unpacked in a folder of its own, outside the repository.
  const work = mkdtempSync(join(tmpdir(), "ambientcg-"));
  try {
    await download(`https://ambientcg.com/get?file=${file}.zip`, join(work, "maps.zip"));
    execFileSync("unzip", ["-q", "-o", join(work, "maps.zip"), "-d", join(work, "maps")]);
    const map = (kind: string) => join(work, "maps", `${file}_${kind}.jpg`);
    for (const kind of ["Color", "NormalGL", "Roughness", "Displacement"]) {
      if (!existsSync(map(kind))) throw new Error(`${id} has no ${kind} map at ${size}`);
    }
    const out = join(ROOT, "assets", "materials", name);
    mkdirSync(out, { recursive: true });
    const jpg = ["-quality", "92"];
    execFileSync(MAGICK,[map("Color"), ...jpg, join(out, "color.jpg")]);
    execFileSync(MAGICK,[map("NormalGL"), ...jpg, join(out, "normal.jpg")]);
    // Occlusion, roughness and no metal, as Bevy reads them.
    const occlusion = existsSync(map("AmbientOcclusion")) ? [map("AmbientOcclusion")] : [map("Roughness"), "-fill", "white", "-colorize", "100"];
    execFileSync(MAGICK,[
      "(", ...occlusion, "-colorspace", "gray", ")",
      "(", map("Roughness"), "-colorspace", "gray", ")",
      "(", map("Roughness"), "-fill", "black", "-colorize", "100", "-colorspace", "gray", ")",
      "-combine", "-colorspace", "sRGB", ...jpg, join(out, "arm.jpg"),
    ]);
    execFileSync(MAGICK,[map("Displacement"), "-colorspace", "gray", "-depth", "8", ...jpg, join(out, "height.jpg")]);
    const info = await (await fetch(`https://ambientcg.com/api/v2/full_json?id=${id}`)).json();
    const title = info.foundAssets?.[0]?.displayName ?? id;
    record(name, `ambientCG [${title}](https://ambientcg.com/a/${id}), CC0, ${size}.`);
    console.log(`${name}: ${out} (ambientCG gives no size in metres; the kit's file says it)`);
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

function stack(name: string, materials: string[], size: number, occlusion: number, height: boolean) {
  const out = join(ROOT, "assets", "materials", name);
  mkdirSync(out, { recursive: true });
  const dir = (m: string) => join(ROOT, "assets", "materials", m);
  const square = ["-resize", `${size}x${size}!`];
  // Colour darkened in its crevices by the occlusion in the ARM map's red.
  const colours = materials.flatMap((m) => [
    "(", join(dir(m), "color.jpg"), ...square,
    "(", join(dir(m), "arm.jpg"), ...square, "-channel", "R", "-separate", "+channel",
    "-evaluate", "multiply", String(occlusion), "-evaluate", "add", String(1 - occlusion), ")",
    "-compose", "Multiply", "-composite", ")",
  ]);
  execFileSync(MAGICK,[...colours, "-append", "-quality", "92", join(out, "color.jpg")]);
  // With --height, each layer's height, stretched to the full range, in place of its normal's
  // blue: the shader works the normal's z out from its x and y, as a two-channel normal map
  // leaves room for, and blends the layers by height. A layer with no height map takes one
  // from its colour's light and dark. Its broad swells are taken out (the height less a blur
  // of it), so one layer gives way to another by its leaves, stones and clods, not in blobs.
  const highPass = ["(", "+clone", "-blur", "0x40", ")", "-compose", "Mathematics", "-define", "compose:args=0,-1,1,0.5", "-composite", "-auto-level"];
  const heightOf = (m: string) =>
    existsSync(join(dir(m), "height.jpg"))
      ? ["(", join(dir(m), "height.jpg"), ...square, "-colorspace", "gray", ...highPass, ")"]
      : ["(", join(dir(m), "color.jpg"), ...square, "-colorspace", "gray", "-blur", "0x2", ...highPass, ")"];
  const normals = materials.flatMap((m) =>
    height
      ? ["(", "(", join(dir(m), "normal.jpg"), ...square, "-separate", "-delete", "2", ")", ...heightOf(m), "-combine", ")"]
      : ["(", join(dir(m), "normal.jpg"), ...square, ")"],
  );
  // No chroma subsampling, so the channels stay apart.
  execFileSync(MAGICK,[...normals, "-append", "-quality", "92", "-sampling-factor", "1x1", join(out, "normal.jpg")]);
  const layers = materials.map((m) => `\`${m}\` (${credited(m)?.replace(/\.$/, "") ?? "uncredited"})`);
  record(name, `a stack, in this order, ${size} pixels each, of ${layers.join("; ")}.`);
  console.log(`${name}: ${materials.length} layers in ${out}`);
}

/** A card atlas's normal map, four bytes a pixel, its alpha each pixel's roughness; one from
 * before roughness was kept in it, opaque, is given `ROUGH` there. */
function normalMap(image: string): { data: Buffer; width: number } {
  const info = execFileSync(MAGICK,[image, "-format", "%w %[channels]", "info:"]).toString().split(" ");
  const data = execFileSync(MAGICK,[image, "-depth", "8", "rgba:-"], { maxBuffer: 1 << 30 });
  if (!info[1].includes("a")) for (let i = 3; i < data.length; i += 4) data[i] = Math.round(ROUGH * 255);
  return { data, width: Number(info[0]) };
}

/** A square of pixels scaled from `from` to `to` pixels across. A colour picture's alpha is
 * its cover, which ImageMagick weights its colour by; any other's four channels are data
 * (a normal map's alpha its roughness), scaled each on its own. */
function resized(data: Buffer, from: number, to: number, kind: "rgba" | "rgb"): Buffer {
  const scale = (input: Buffer, format: string) =>
    execFileSync(MAGICK,["-size", `${from}x${from}`, "-depth", "8", `${format}:-`, "-filter", "Lanczos", "-resize", `${to}x${to}!`, `${format}:-`], {
      input,
      maxBuffer: 1 << 30,
    });
  if (kind === "rgba") return scale(data, "rgba");
  const [rgb, alpha] = [Buffer.alloc(from * from * 3), Buffer.alloc(from * from)];
  for (let i = 0; i < from * from; i++) {
    data.copy(rgb, i * 3, i * 4, i * 4 + 3);
    alpha[i] = data[i * 4 + 3];
  }
  const [big, bigAlpha] = [scale(rgb, "rgb"), scale(alpha, "gray")];
  const out = Buffer.alloc(to * to * 4);
  for (let i = 0; i < to * to; i++) {
    big.copy(out, i * 4, i * 3, i * 3 + 3);
    out[i * 4 + 3] = bigAlpha[i];
  }
  return out;
}

/** An image's pixels, four bytes each, and its size. */
function rgba(image: string, size?: number): { data: Buffer; width: number; height: number } {
  const resize = size ? ["-resize", `${size}x${size}!`] : [];
  const info = execFileSync(MAGICK,[image, ...resize, "-format", "%w %h", "info:"]).toString().split(" ");
  const data = execFileSync(MAGICK,[image, ...resize, "-alpha", "on", "-depth", "8", "rgba:-"], {
    maxBuffer: 1 << 28,
  });
  return { data, width: Number(info[0]), height: Number(info[1]) };
}

/** Cuts a picture drawn on a flat blue background out of it, as a chroma key does: each
 * pixel's cover from how much bluer it is than its other channels against the
 * background's own blueness, and its colour unmixed from the background's by that cover,
 * so edges carry no blue. A pixel the picture does not cover takes the colour of the
 * nearest that it does, so filtering at the edge of a leaf reads leaf. */
export function keyOut(data: Buffer, width: number, height: number): Buffer {
  const n = width * height;
  const blueness = (i: number) => data[i * 4 + 2] - Math.max(data[i * 4], data[i * 4 + 1]);
  // The background, from the border, which a card's picture leaves clear.
  const border: number[] = [];
  for (let x = 0; x < width; x++) border.push(x, (height - 1) * width + x);
  for (let y = 0; y < height; y++) border.push(y * width, y * width + width - 1);
  border.sort((a, b) => blueness(a) - blueness(b));
  const bg = border[Math.floor(border.length / 2)];
  const back = [data[bg * 4], data[bg * 4 + 1], data[bg * 4 + 2]];
  const full = Math.max(1, blueness(bg));
  const out = Buffer.alloc(n * 4);
  const known = new Uint8Array(n);
  for (let i = 0; i < n; i++) {
    // The cover that leaves the unmixed colour no bluer than grey, as leaves, stems and petals
    // are; what is a little bluish, as a white petal can be, counts as wholly covered.
    const t = (blueness(i) - 0.04 * full) / (0.96 * full);
    const cover = Math.min(1, Math.max(0, 1 - t));
    out[i * 4 + 3] = Math.round(cover * 255);
    if (cover > 0.05) {
      for (let c = 0; c < 3; c++) {
        const unmixed = (data[i * 4 + c] - (1 - cover) * back[c]) / cover;
        out[i * 4 + c] = Math.round(Math.min(255, Math.max(0, unmixed)));
      }
      // What blue is left is the background's: no more than the other channels have.
      out[i * 4 + 2] = Math.min(out[i * 4 + 2], Math.max(out[i * 4], out[i * 4 + 1]));
      known[i] = cover > 0.5 ? 1 : 0;
    }
  }
  // Colour grown outward into what is not covered, a pixel a pass.
  let front = known.slice();
  for (let pass = 0; pass < 24; pass++) {
    const next = front.slice();
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const i = y * width + x;
        if (front[i]) continue;
        const sum = [0, 0, 0];
        let count = 0;
        for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
          const [nx, ny] = [x + dx, y + dy];
          if (nx < 0 || ny < 0 || nx >= width || ny >= height || !front[ny * width + nx]) continue;
          for (let c = 0; c < 3; c++) sum[c] += out[(ny * width + nx) * 4 + c];
          count++;
        }
        if (count) {
          for (let c = 0; c < 3; c++) out[i * 4 + c] = Math.round(sum[c] / count);
          next[i] = 1;
        }
      }
    }
    front = next;
  }
  // Beyond that, the average of what is covered.
  const mean = [0, 0, 0];
  let covered = 0;
  for (let i = 0; i < n; i++) {
    if (!known[i]) continue;
    for (let c = 0; c < 3; c++) mean[c] += out[i * 4 + c];
    covered++;
  }
  for (let i = 0; i < n; i++) {
    if (front[i]) continue;
    for (let c = 0; c < 3; c++) out[i * 4 + c] = Math.round(mean[c] / Math.max(1, covered));
  }
  return out;
}

/** A cut-out picture with the colour of what it covers wholly grown outward, a pixel a
 * pass, over its edge and everything clear, its transparency kept (alpha dilation, as
 * shipped games' foliage textures have it): from pixels opaque and a pixel inside the edge,
 * so the edge's pale rim goes too, and filtering or mipmaps at a card's edge read leaf,
 * not black or sky. Beyond 16 passes, the average of what is covered. */
export function bleed(data: Buffer, width: number, height: number): Buffer {
  const n = width * height;
  const out = Buffer.from(data);
  const opaque = (x: number, y: number) => x < 0 || y < 0 || x >= width || y >= height || data[(y * width + x) * 4 + 3] >= 250;
  let front = new Uint8Array(n);
  const mean = [0, 0, 0];
  let covered = 0;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (opaque(x, y) && opaque(x + 1, y) && opaque(x - 1, y) && opaque(x, y + 1) && opaque(x, y - 1)) {
        const i = y * width + x;
        front[i] = 1;
        for (let c = 0; c < 3; c++) mean[c] += data[i * 4 + c];
        covered++;
      }
    }
  }
  for (let pass = 0; pass < 16; pass++) {
    const next = front.slice();
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        const i = y * width + x;
        if (front[i]) continue;
        const sum = [0, 0, 0];
        let count = 0;
        for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1], [1, 1], [-1, -1], [1, -1], [-1, 1]]) {
          const [nx, ny] = [x + dx, y + dy];
          if (nx < 0 || ny < 0 || nx >= width || ny >= height || !front[ny * width + nx]) continue;
          for (let c = 0; c < 3; c++) sum[c] += out[(ny * width + nx) * 4 + c];
          count++;
        }
        if (count) {
          for (let c = 0; c < 3; c++) out[i * 4 + c] = Math.round(sum[c] / count);
          next[i] = 1;
        }
      }
    }
    front = next;
  }
  for (let i = 0; i < n; i++) {
    if (front[i]) continue;
    for (let c = 0; c < 3; c++) out[i * 4 + c] = Math.round(mean[c] / Math.max(1, covered));
  }
  return out;
}

/** The octagon round every pixel covered past `cut`, as corners in the image's own
 * coordinates from 0 to 1, clockwise from the top left: the tightest with sides along the
 * axes and the diagonals, a pixel out. A card cut to it shades far fewer empty pixels
 * than a square, for six triangles instead of two (Persson's particle trimming). */
export function outline(data: Buffer, width: number, height: number, cut: number): [number, number][] {
  // Furthest a covered pixel reaches along x, y, x + y and x - y, each way.
  let [minX, maxX, minY, maxY] = [Infinity, -Infinity, Infinity, -Infinity];
  let [minS, maxS, minD, maxD] = [Infinity, -Infinity, Infinity, -Infinity];
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (data[(y * width + x) * 4 + 3] <= cut * 255) continue;
      // The pixel's corners, so the octagon holds the whole of it.
      for (const [px, py] of [[x, y], [x + 1, y], [x, y + 1], [x + 1, y + 1]]) {
        minX = Math.min(minX, px);
        maxX = Math.max(maxX, px);
        minY = Math.min(minY, py);
        maxY = Math.max(maxY, py);
        minS = Math.min(minS, px + py);
        maxS = Math.max(maxS, px + py);
        minD = Math.min(minD, px - py);
        maxD = Math.max(maxD, px - py);
      }
    }
  }
  if (minX === Infinity) return [];
  const m = 1;
  [minX, maxX, minY, maxY] = [minX - m, maxX + m, minY - m, maxY + m];
  [minS, maxS, minD, maxD] = [minS - 2 * m, maxS + 2 * m, minD - 2 * m, maxD + 2 * m];
  // Corners where each side meets the next, y growing downward.
  const corners: [number, number][] = [
    [minS - minY, minY], // top, from the top-left diagonal
    [maxD + minY, minY], // top, to the top-right diagonal
    [maxX, maxX - maxD], // right, from the top-right diagonal
    [maxX, maxS - maxX], // right, to the bottom-right diagonal
    [maxS - maxY, maxY], // bottom, from the bottom-right diagonal
    [minD + maxY, maxY], // bottom, to the bottom-left diagonal
    [minX, minX - minD], // left, from the bottom-left diagonal
    [minX, minS - minX], // left, to the top-left diagonal
  ];
  const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
  return corners.map(([x, y]) => [
    Number((clamp(x, minX, maxX) / width).toFixed(4)),
    Number((clamp(y, minY, maxY) / height).toFixed(4)),
  ]);
}

/** A card's picture cut out of its blue background, `cell` pixels square, four bytes a pixel. */
function pictureCell(image: string, cell: number): Buffer {
  const source = rgba(resolve(image));
  // A picture already cut out, with its own transparency, is taken as it is.
  let cutAlready = false;
  for (let i = 3; i < source.data.length && !cutAlready; i += 4) cutAlready = source.data[i] < 255;
  const keyed = cutAlready ? source.data : keyOut(source.data, source.width, source.height);
  return execFileSync(MAGICK,["-size", `${source.width}x${source.height}`, "-depth", "8", "rgba:-", "-resize", `${cell}x${cell}!`, "rgba:-"], {
    input: keyed,
    maxBuffer: 1 << 28,
  });
}

/** An ambientCG leaf set's maps at 2K, downloaded once into the system's temporary folder
 * and unpacked in a folder of its own, outside the repository. */
async function leafScan(id: string): Promise<Scan> {
  const file = `${id}_2K-PNG`;
  const cache = join(tmpdir(), "terra-ambientcg");
  mkdirSync(cache, { recursive: true });
  const zip = join(cache, `${file}.zip`);
  if (!existsSync(zip)) await download(`https://ambientcg.com/get?file=${file}.zip`, zip);
  const work = mkdtempSync(join(tmpdir(), "leafset-"));
  try {
    execFileSync("unzip", ["-q", "-o", zip, "-d", work]);
    const map = (kind: string) => join(work, `${file}_${kind}.png`);
    // At the scan's own size: a 2K set's leaves are 400 to 550 pixels long, two to four times
    // what a card draws them at.
    const n = 2048;
    const read = (kind: string, format: string) =>
      execFileSync(MAGICK,[map(kind), "-resize", `${n}x${n}!`, "-depth", "8", format], { maxBuffer: 1 << 28 });
    const roughness = existsSync(map("Roughness")) ? read("Roughness", "gray:-") : undefined;
    return { width: n, height: n, colour: read("Color", "rgb:-"), normal: read("NormalGL", "rgb:-"), alpha: read("Opacity", "gray:-"), roughness };
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}

/** Where each card's cell is in the card table as it stands, by name. */
export function cellsOf(ron: string): Map<string, [number, number]> {
  const cells = new Map<string, [number, number]>();
  for (const [, card, u, v] of ron.matchAll(/"(\w+)": \(\s*cell: \(([\d.]+), ([\d.]+)\)/g)) cells.set(card, [Number(u), Number(v)]);
  return cells;
}

/** How long each card's leaves are, as a share of its width, in the card table as it stands,
 * by name, for those that say. */
export function leavesOf(ron: string): Map<string, number> {
  const leaves = new Map<string, number>();
  const starts = [...ron.matchAll(/"(\w+)": \(/g)];
  starts.forEach((start, k) => {
    const entry = ron.slice(start.index, starts[k + 1]?.index ?? ron.length);
    const leaf = entry.match(/leaf: ([\d.]+)/);
    if (leaf) leaves.set(start[1], Number(leaf[1]));
  });
  return leaves;
}

/** Lays each card in a square atlas, a grid of `columns` cells, with a normal map beside it,
 * and writes the card table the game reads, which gives each card's cell and outline in the
 * atlas's coordinates. A card is one of: a picture, cut out of its blue background (flat in
 * the normal map); a recipe (`.leaves.json`), composed from scanned leaves (leaves.ts); or
 * `keep`, its cell as the atlas has it now, for one touched by hand since. */
async function cards(name: string, entries: string[], size: number, columns: number, cut: number) {
  const out = join(ROOT, "assets", "materials", name);
  mkdirSync(out, { recursive: true });
  const cell = Math.floor(size / columns);
  const atlas = Buffer.alloc(size * size * 4);
  // The normal map, its alpha each pixel's roughness (D292).
  const normals = Buffer.alloc(size * size * 4);
  const flat = [128, 128, 255, Math.round(ROUGH * 255)];
  for (let i = 0; i < size * size; i++) normals.set(flat, i * 4);
  const ronFile = join(ROOT, "assets", "data", `${name}.ron`);
  const standing = existsSync(ronFile) ? readFileSync(ronFile, "utf8") : "";
  const before = cellsOf(standing);
  const leavesBefore = leavesOf(standing);
  const old = existsSync(join(out, "cards.png")) ? rgba(join(out, "cards.png")) : undefined;
  // A normal map from before roughness was in its alpha has none: as rough as `ROUGH` there.
  const oldNormals = existsSync(join(out, "normal.png")) ? normalMap(join(out, "normal.png")) : undefined;
  const table: string[] = [];
  const scans = new Map<string, Scan>();
  const composed: string[] = [];
  const kept: string[] = [];
  const pictured: string[] = [];
  for (const [index, entry] of entries.entries()) {
    const [card, source] = entry.split("=");
    if (!card || !source) throw new Error(`a card is name=image, name=recipe.leaves.json or name=keep, not ${entry}`);
    let pixels: Buffer;
    let normal: Buffer;
    let leaf = leavesBefore.get(card);
    if (source === "keep") {
      const at = before.get(card);
      if (!at || !old) throw new Error(`${card} is not in the atlas as it stands, to keep`);
      // Its cell as it stands, scaled to this atlas's cells if they are another size.
      const was = Math.floor(old.width / columns);
      const [ox, oy] = [Math.round(at[0] * old.width), Math.round(at[1] * old.width)];
      const from = (data: Buffer, kind: "rgba" | "rgb") => {
        const piece = Buffer.alloc(was * was * 4);
        for (let y = 0; y < was; y++) data.copy(piece, y * was * 4, ((oy + y) * old.width + ox) * 4, ((oy + y) * old.width + ox + was) * 4);
        return was === cell ? piece : resized(piece, was, cell, kind);
      };
      pixels = from(old.data, "rgba");
      normal = Buffer.alloc(cell * cell * 4);
      for (let i = 0; i < cell * cell; i++) normal.set(flat, i * 4);
      if (oldNormals && oldNormals.width === old.width) normal = from(oldNormals.data, "rgb");
      kept.push(card);
    } else if (source.endsWith(".leaves.json")) {
      const recipe: Recipe = JSON.parse(readFileSync(resolve(source), "utf8"));
      // A conifer's needles are drawn, from no scan.
      const drawn = recipe.form === "needles" || recipe.form === "blades";
      const id = drawn ? undefined : recipe.scan;
      if (id !== undefined && !scans.has(id)) scans.set(id, await leafScan(id));
      const made = compose(id === undefined ? undefined : scans.get(id), recipe, cell);
      leaf = made.leaf;
      // The colour grown out over its edge, as for a picture.
      pixels = bleed(made.rgba, cell, cell);
      normal = Buffer.alloc(cell * cell * 4);
      for (let i = 0; i < cell * cell; i++) normal.set([made.normal[i * 3], made.normal[i * 3 + 1], made.normal[i * 3 + 2], made.rough[i]], i * 4);
      composed.push(drawn ? `${card} (drawn needle by needle)` : `${card} (${recipe.scan})`);
    } else {
      leaf = undefined;
      // Resizing leaves what is clear black, and the edge a pale rim, which filtering at a
      // card's edge mixes in as a fringe; the colour inside is grown out over both.
      pixels = bleed(pictureCell(source, cell), cell, cell);
      // Flat, and as rough as a file beside the picture says (`<card>.surface.json`), such as
      // holly's waxy leaves; `ROUGH` otherwise.
      const surface = source.replace(/\.png$/, ".surface.json");
      const rough = existsSync(surface) ? Number(JSON.parse(readFileSync(surface, "utf8")).roughness) : ROUGH;
      normal = Buffer.alloc(cell * cell * 4);
      for (let i = 0; i < cell * cell; i++) normal.set([128, 128, 255, Math.round(rough * 255)], i * 4);
      pictured.push(card);
    }
    const [col, row] = [index % columns, Math.floor(index / columns)];
    for (let y = 0; y < cell; y++) {
      pixels.copy(atlas, ((row * cell + y) * size + col * cell) * 4, y * cell * 4, (y + 1) * cell * 4);
      normal.copy(normals, ((row * cell + y) * size + col * cell) * 4, y * cell * 4, (y + 1) * cell * 4);
    }
    const shape = outline(pixels, cell, cell, cut);
    const colour = meanColour(pixels, cut);
    const corner = (u: number, v: number) => `(${((col + u) / columns).toFixed(4)}, ${((row + v) / columns).toFixed(4)})`;
    table.push(`        "${card}": (`);
    table.push(`            cell: ${corner(0, 0)},`);
    table.push(`            outline: [${shape.map(([u, v]) => corner(u, v)).join(", ")}],`);
    table.push(`            colour: (${colour.map((c) => c.toFixed(3)).join(", ")}),`);
    if (leaf !== undefined) table.push(`            leaf: ${leaf.toFixed(4)},`);
    table.push(`        ),`);
    const leaves = leaf === undefined ? "" : `, its leaves ${(100 * leaf).toFixed(1)}% of it long`;
    console.log(`${card}: cell ${col},${row}, its octagon ${(100 * area(shape)).toFixed(0)}% of the cell, colour ${colour.map((c) => c.toFixed(2))}${leaves}`);
  }
  execFileSync(MAGICK,["-size", `${size}x${size}`, "-depth", "8", "rgba:-", join(out, "cards.png")], { input: atlas });
  execFileSync(MAGICK,["-size", `${size}x${size}`, "-depth", "8", "rgba:-", join(out, "normal.png")], { input: normals });
  const ron = [
    `// The leaf, frond and flower cards in assets/materials/${name}/cards.png, made from their`,
    `// pictures and recipes by \`node tools/textures/textures.ts cards\`; don't edit by hand. Each`,
    `// card's cell is the top-left corner of its square in the atlas, a ${columns}th of it across, and`,
    `// its outline the octagon round what it covers past the cut, in the atlas's coordinates (0 to`,
    `// 1), its colour the average of what it covers, in sRGB, and for a composed card how long its`,
    `// leaves are on average, as a share of the card's width. The normal map beside it is each`,
    `// leaf's relief and tilt, in OpenGL's convention; flat for cards made from pictures.`,
    `(`,
    `    atlas: "materials/${name}/cards.png",`,
    `    normals: "materials/${name}/normal.png",`,
    `    cut: ${cut},`,
    `    across: ${(1 / columns).toFixed(4)},`,
    `    cards: {`,
    ...table,
    `    },`,
    `)`,
    ``,
  ];
  writeFileSync(ronFile, ron.join("\n"));
  const scanned = [...scans.keys()].map((id) => `[${id}](https://ambientcg.com/a/${id})`).join(", ");
  const parts = [
    composed.length &&
      (scans.size
        ? `${composed.join(", ")} composed by \`textures.ts cards\` from leaves scanned by ambientCG, ${scanned}, CC0, each with its normal map`
        : `${composed.join(", ")} composed by \`textures.ts cards\`, each with its normal map`),
    pictured.length && `${pictured.join(", ")} each cut out of a generated picture on a blue background (art/textures/cards)`,
    kept.length && `${kept.join(", ")} kept from the atlas as it stood`,
  ].filter(Boolean);
  record(name, `cards, ${cell} pixels each: ${parts.join("; ")}.`);
}

/** The average colour of what a card covers, in sRGB from 0 to 1, averaged as light adds:
 * what it reads as from far off, where a tree is drawn as solid lumps in it. */
export function meanColour(data: Buffer, cut: number): [number, number, number] {
  const linear = (c: number) => {
    const v = c / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  const encoded = (v: number) => (v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055);
  const sum = [0, 0, 0];
  let count = 0;
  for (let i = 0; i < data.length; i += 4) {
    if (data[i + 3] <= cut * 255) continue;
    for (let c = 0; c < 3; c++) sum[c] += linear(data[i + c]);
    count++;
  }
  return sum.map((v) => encoded(v / Math.max(1, count))) as [number, number, number];
}

/** A map's pixels, `channels` bytes each (1 or 3), scaled to a size, or tiled across it from
 * copies `tile` pixels big when given. */
function pixels(image: string, channels: 1 | 3, width: number, height: number, tile?: [number, number]): Buffer {
  const format = channels === 1 ? "gray:-" : "rgb:-";
  const grey = channels === 1 ? ["-colorspace", "gray"] : [];
  const args = tile
    ? ["(", image, "-resize", `${tile[0]}x${tile[1]}!`, "-write", "mpr:copy", "+delete", ")",
       "-size", `${width}x${height}`, "tile:mpr:copy"]
    : [image, "-resize", `${width}x${height}!`];
  return execFileSync(MAGICK,[...args, ...grey, "-depth", "8", format], { maxBuffer: 1 << 28 });
}

function save(data: Buffer, channels: 1 | 3, width: number, height: number, to: string) {
  execFileSync(MAGICK,["-size", `${width}x${height}`, "-depth", "8", channels === 1 ? "gray:-" : "rgb:-", "-quality", "92", to], {
    input: data,
  });
}

/** Smooth noise from 0 to 1 that repeats across a `width` by `height` image, `cells` lattice
 * cells across and down it, summed over `octaves`, each twice as fine and half as strong:
 * a mask that tiles as the texture it shapes does. */
export function tileNoise(width: number, height: number, cells: [number, number], seed: number, octaves = 3): Float32Array {
  const out = new Float32Array(width * height);
  const hash = (x: number, y: number, o: number) => {
    let h = (x * 374761393 + y * 668265263 + o * 2147483647 + seed * 974711) | 0;
    h = Math.imul(h ^ (h >>> 13), 1274126177);
    return ((h ^ (h >>> 16)) >>> 0) / 4294967295;
  };
  let total = 0;
  for (let o = 0; o < octaves; o++) {
    const [cx, cy] = [cells[0] << o, cells[1] << o];
    const weight = 1 / (1 << o);
    total += weight;
    for (let y = 0; y < height; y++) {
      const fy = (y / height) * cy;
      const y0 = Math.floor(fy);
      const ty = fy - y0;
      const sy = ty * ty * (3 - 2 * ty);
      for (let x = 0; x < width; x++) {
        const fx = (x / width) * cx;
        const x0 = Math.floor(fx);
        const tx = fx - x0;
        const sx = tx * tx * (3 - 2 * tx);
        const at = (i: number, j: number) => hash(i % cx, j % cy, o);
        const top = at(x0, y0) * (1 - sx) + at(x0 + 1, y0) * sx;
        const bottom = at(x0, y0 + 1) * (1 - sx) + at(x0 + 1, y0 + 1) * sx;
        out[y * width + x] += weight * (top * (1 - sy) + bottom * sy);
      }
    }
  }
  for (let i = 0; i < out.length; i++) out[i] /= total;
  return out;
}

const smoothstep = (a: number, b: number, x: number) => {
  const t = Math.min(1, Math.max(0, (x - a) / (b - a)));
  return t * t * (3 - 2 * t);
};

/** **blend**: a log's face, one material lying over another in patches, as bark stays on a
 * log in places where the rest has weathered off: `over` (the bark) on `base` (the bare,
 * weathered wood), each tiled at its own size in metres, over a texture `around` metres
 * round the log by `along` metres along it, the grain and the patches running along it.
 * Where the bark lies is tileable noise drawn out along the log, its edge ragged by the
 * bark's own height; the bark stands proud of the wood in height and normal, with a dark
 * lip where it ends. The texture is square, so a long one is squeezed along its length and
 * the kit's `metres` stretches it back. */
function blend(
  name: string,
  base: string,
  over: string,
  {
    around,
    along,
    cover,
    size,
    seed,
    saturation,
    lighten,
    exposed,
    soft,
    woodRoughness,
  }: {
    around: number;
    along: number;
    cover: number;
    size: number;
    seed: number;
    saturation: number;
    lighten: number;
    exposed: boolean;
    soft: number;
    woodRoughness?: number;
  },
) {
  const out = join(ROOT, "assets", "materials", name);
  mkdirSync(out, { recursive: true });
  const dir = (m: string) => join(ROOT, "assets", "materials", m);
  const parse = (entry: string): [string, number] => {
    const [material, metres] = entry.split("=");
    if (!material || !Number(metres)) throw new Error(`a layer is material=metres, not ${entry}`);
    return [material, Number(metres)];
  };
  const [[baseName, baseM], [overName, overM]] = [parse(base), parse(over)];
  const [w, h] = [size, size];
  const tile = (metres: number): [number, number] => [Math.round((w * metres) / around), Math.round((h * metres) / along)];
  const layer = (material: string, metres: number) => ({
    colour: pixels(join(dir(material), "color.jpg"), 3, w, h, tile(metres)),
    normal: pixels(join(dir(material), "normal.jpg"), 3, w, h, tile(metres)),
    arm: pixels(join(dir(material), "arm.jpg"), 3, w, h, tile(metres)),
    height: existsSync(join(dir(material), "height.jpg"))
      ? pixels(join(dir(material), "height.jpg"), 1, w, h, tile(metres))
      : Buffer.alloc(w * h, 128),
  });
  const [wood, bark] = [layer(baseName, baseM), layer(overName, overM)];
  // The bark's colour toward its grey by `saturation`, as bark on a felled log fades; the
  // bare wood lighter by `lighten`, as wood left in the sun goes silver.
  for (let i = 0; i < w * h * 3; i++) wood.colour[i] = Math.min(255, Math.round(wood.colour[i] * lighten));
  // And rougher, as wood weathered silver is: its roughness moved to `woodRoughness` on
  // average, keeping its grain's variation. Sawn wood's scan is half rough, which caught the
  // sky as a blue sheen on every log's silvered top at noon.
  if (woodRoughness !== undefined) {
    let sum = 0;
    for (let i = 0; i < w * h; i++) sum += wood.arm[i * 3 + 1];
    const shift = woodRoughness * 255 - sum / (w * h);
    for (let i = 0; i < w * h; i++) wood.arm[i * 3 + 1] = Math.max(0, Math.min(255, Math.round(wood.arm[i * 3 + 1] + shift)));
  }
  for (let i = 0; i < w * h; i++) {
    const [r, g, b] = [bark.colour[i * 3], bark.colour[i * 3 + 1], bark.colour[i * 3 + 2]];
    const luma = 0.3 * r + 0.55 * g + 0.15 * b;
    for (let c = 0; c < 3; c++) bark.colour[i * 3 + c] = Math.round(luma + (bark.colour[i * 3 + c] - luma) * saturation);
  }
  // Patches about a sixth of the way round and two thirds of a metre along, finer
  // octaves breaking their edges.
  const cells: [number, number] = [Math.max(1, Math.round(around * 6)), Math.max(1, Math.round(along * 1.5))];
  const noise = tileNoise(w, h, cells, seed, 4);
  // With `exposed`, the texture is laid once round each log from underneath (D262): a
  // quarter of the way across is its face to the weather, half way its top, three quarters
  // its face in the lee. Bark is lost where a log is exposed, on its top and the upper half
  // of its weather face, a little on the upper half of its lee face, and never underneath.
  const exposure = (x: number) =>
    exposed
      ? Math.exp(-(((x - 0.5) / 0.11) ** 2)) +
        0.8 * Math.exp(-(((x - 0.33) / 0.09) ** 2)) +
        0.35 * Math.exp(-(((x - 0.67) / 0.08) ** 2))
      : 0;
  // How much bark each pixel keeps, highest where the noise is and the log is sheltered.
  const keep = new Float32Array(w * h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = y * w + x;
      keep[i] = noise[i] - 0.3 * exposure((x + 0.5) / w);
    }
  }
  // The level that leaves `cover` of the texture in bark.
  const sorted = Float32Array.from(keep).sort();
  const level = sorted[Math.min(sorted.length - 1, Math.floor((1 - cover) * sorted.length))];
  const mask = new Float32Array(w * h);
  for (let i = 0; i < w * h; i++) {
    const ragged = (bark.height[i] / 255 - 0.5) * 0.12;
    mask[i] = smoothstep(level - soft, level + soft, keep[i] + ragged);
  }
  const colour = Buffer.alloc(w * h * 3);
  const normal = Buffer.alloc(w * h * 3);
  const arm = Buffer.alloc(w * h * 3);
  const height = Buffer.alloc(w * h);
  // How thick the bark reads in the normal map where it ends, in texels of height.
  const lip = 6;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = y * w + x;
      const m = mask[i];
      // The bark's edge: how fast the mask changes across and along, for its lip.
      const at = (dx: number, dy: number) => mask[((y + dy + h) % h) * w + ((x + dx + w) % w)];
      const [gx, gy] = [(at(2, 0) - at(-2, 0)) / 4, (at(0, 2) - at(0, -2)) / 4];
      const edge = Math.min(1, Math.hypot(gx, gy) * 8);
      // In the wood just off the bark's edge, a shadow under its lip.
      const shadow = 1 - (exposed ? 0.18 : 0.35) * edge * (1 - m);
      for (let c = 0; c < 3; c++) {
        const k = i * 3 + c;
        colour[k] = Math.round((wood.colour[k] * (1 - m) + bark.colour[k] * m) * shadow);
        arm[k] = Math.round(wood.arm[k] * (1 - m) + bark.arm[k] * m);
      }
      arm[i * 3] = Math.round(arm[i * 3] * shadow);
      // The wood lies low, the bark stands on it.
      const hw = wood.height[i] / 255;
      const hb = bark.height[i] / 255;
      height[i] = Math.round(255 * (hw * 0.4 * (1 - m) + (0.45 + 0.55 * hb) * m));
      // Occluded as low as it lies, as an occlusion map baked from the height would be: the
      // bare wood between the bark's plates, and the plates' own furrows, in shade.
      arm[i * 3] = Math.round(arm[i * 3] * (0.6 + 0.4 * (height[i] / 255)));
      // Normals blended, then tilted by the bark's lip, OpenGL's way up (green up the image).
      const n = [0, 1, 2].map((c) => ((wood.normal[i * 3 + c] * (1 - m) + bark.normal[i * 3 + c] * m) / 255) * 2 - 1);
      n[0] -= gx * lip;
      n[1] += gy * lip;
      const length = Math.hypot(n[0], n[1], n[2]) || 1;
      for (let c = 0; c < 3; c++) normal[i * 3 + c] = Math.round(((n[c] / length) * 0.5 + 0.5) * 255);
    }
  }
  save(colour, 3, w, h, join(out, "color.jpg"));
  save(normal, 3, w, h, join(out, "normal.jpg"));
  save(arm, 3, w, h, join(out, "arm.jpg"));
  save(height, 1, w, h, join(out, "height.jpg"));
  const credit = (m: string) => `\`${m}\` (${credited(m)?.replace(/\.$/, "") ?? "uncredited"})`;
  record(
    name,
    `${credit(overName)} at ${overM} m lying in patches over ${credit(baseName)} at ${baseM} m, ` +
      `${Math.round(cover * 100)}% covered, its colour ${Math.round(saturation * 100)}% saturated and the wood's ${Math.round(lighten * 100)}% as light${woodRoughness !== undefined ? ` and ${woodRoughness} rough` : ""}${exposed ? ", the bark lost where a log is exposed" : ""}, by \`textures.ts blend\`. ${around.toFixed(2)} m round by ${along.toFixed(2)} m along.`,
  );
  console.log(`${name}: ${out}, ${around} m round by ${along} m along, ${Math.round(cover * 100)}% ${overName}`);
}

/** Where a cut-out's edge is from a centre, every degree round, from its opacity: how far
 * out the last covered pixel along each ray lies, smoothed over its neighbours. */
export function rim(alpha: Buffer, width: number, height: number, centre: [number, number]): number[] {
  const reach = Math.hypot(width, height) / 2;
  const raw = Array.from({ length: 360 }, (_, d) => {
    const a = (d * Math.PI) / 180;
    let last = 0;
    for (let r = 0; r < reach; r += 0.5) {
      const [x, y] = [Math.round(centre[0] + Math.cos(a) * r), Math.round(centre[1] + Math.sin(a) * r)];
      if (x < 0 || y < 0 || x >= width || y >= height) break;
      if (alpha[y * width + x] > 127) last = r;
    }
    return last;
  });
  return raw.map((_, d) => {
    let sum = 0;
    for (let k = -3; k <= 3; k++) sum += raw[(d + k + 360) % 360];
    return sum / 7;
  });
}

/** A small seeded random number generator (Mulberry32), from 0 up to 1. */
function mulberry(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** A number from a name, the same every run. */
function zlibish(text: string): number {
  let h = 2166136261;
  for (const ch of text) h = Math.imul(h ^ ch.charCodeAt(0), 16777619);
  return h >>> 0;
}

/** **ends**: a log's sawn ends, an atlas of real tree ends scanned by ambientCG (CC0), each
 * cut out by its opacity and drawn out round so its bark's edge falls on a circle 0.47 of its
 * cell from the middle, as a log's cap maps its rim; greyed by `weather` as an end left in the
 * weather goes; opened by `checks`, radial cracks from its rim in, as an end does drying.
 * Cells side by side, `columns` across; a log draws one by its seed. */
async function ends(name: string, ids: string[], size: number, weather: number, checks: number) {
  const out = join(ROOT, "assets", "materials", name);
  const tmp = join(out, ".scans");
  mkdirSync(tmp, { recursive: true });
  const columns = Math.ceil(Math.sqrt(ids.length));
  const cell = Math.floor(size / columns);
  const maps = { colour: Buffer.alloc(size * size * 3), normal: Buffer.alloc(size * size * 3), arm: Buffer.alloc(size * size * 3) };
  for (const [index, id] of ids.entries()) {
    const zip = join(tmp, `${id}.zip`);
    if (!existsSync(zip)) await download(`https://ambientcg.com/get?file=${ambientCgFile(id, "2k")}.zip`, zip);
    execFileSync("unzip", ["-o", "-q", zip, "-d", join(tmp, id)]);
    const file = (map: string) => join(tmp, id, `${ambientCgFile(id, "2k")}_${map}.jpg`);
    const n = 1024;
    const alpha = pixels(file("Opacity"), 1, n, n);
    let [sx, sy, count] = [0, 0, 0];
    for (let i = 0; i < n * n; i++) {
      if (alpha[i] > 127) {
        sx += i % n;
        sy += Math.floor(i / n);
        count++;
      }
    }
    if (!count) throw new Error(`${id} has nothing in its opacity`);
    const centre: [number, number] = [sx / count, sy / count];
    const edge = rim(alpha, n, n, centre);
    const colour = pixels(file("Color"), 3, n, n);
    const normal = pixels(file("NormalGL"), 3, n, n);
    const rough = pixels(file("Roughness"), 1, n, n);
    const occlusion = existsSync(file("AmbientOcclusion")) ? pixels(file("AmbientOcclusion"), 1, n, n) : Buffer.alloc(n * n, 255);
    const [col, row] = [index % columns, Math.floor(index / columns)];
    // Checks: the radial cracks a log's end opens as it dries, from its rim in, each at its
    // own angle, as deep as its seed says, wandering a little and narrowing inward.
    const rng = mulberry(zlibish(id));
    const cracks = Array.from({ length: checks }, () => ({
      angle: rng() * 2 * Math.PI,
      from: 0.25 + 0.55 * rng(),
      wide: 0.003 + 0.004 * rng(),
      wander: rng() * 10,
    }));
    for (let y = 0; y < cell; y++) {
      for (let x = 0; x < cell; x++) {
        const [dx, dy] = [(x + 0.5) / cell - 0.5, (y + 0.5) / cell - 0.5];
        const rho = Math.min(0.985, Math.hypot(dx, dy) / 0.47);
        const angle = Math.atan2(dy, dx);
        const degree = ((angle * 180) / Math.PI + 360) % 360;
        const d0 = Math.floor(degree);
        const t = degree - d0;
        const reach = edge[d0 % 360] * (1 - t) + edge[(d0 + 1) % 360] * t;
        const [u, v] = [centre[0] + Math.cos(angle) * rho * reach, centre[1] + Math.sin(angle) * rho * reach];
        const [i0, j0] = [Math.min(n - 1, Math.max(0, Math.round(u))), Math.min(n - 1, Math.max(0, Math.round(v)))];
        const from = j0 * n + i0;
        const to = (row * cell + y) * size + col * cell + x;
        const luma = (colour[from * 3] * 0.3 + colour[from * 3 + 1] * 0.55 + colour[from * 3 + 2] * 0.15) / 255;
        // Weathered: towards a grey a little warm, a little darker.
        const grey = [luma * 0.86, luma * 0.82, luma * 0.76];
        // How far into a check this pixel is, 1 at its middle, and which way across it.
        let crack = 0;
        let across = 0;
        const r = Math.hypot(dx, dy) / 0.47;
        for (const k of cracks) {
          if (r < k.from || r > 1.0) continue;
          const turn = Math.atan2(Math.sin(angle - k.angle - 0.012 * Math.sin(r * 7 + k.wander)), Math.cos(angle - k.angle - 0.012 * Math.sin(r * 7 + k.wander)));
          const half = k.wide * Math.min(1, (r - k.from) / 0.25);
          const d = Math.abs(turn) * r * 0.47;
          if (d < half) {
            const depth = 1 - d / half;
            if (depth > crack) {
              crack = depth;
              across = Math.sign(turn);
            }
          }
        }
        for (let c = 0; c < 3; c++) {
          const fresh = colour[from * 3 + c] / 255;
          const weathered = fresh * (1 - weather) + grey[c] * weather;
          maps.colour[to * 3 + c] = Math.round(255 * weathered * (1 - 0.6 * crack));
          maps.normal[to * 3 + c] = normal[from * 3 + c];
        }
        if (crack > 0) {
          // The check's walls lean in to its middle, across the radius.
          const [tx, ty] = [-Math.sin(angle) * across, Math.cos(angle) * across];
          const n0 = [0, 1, 2].map((c) => (normal[from * 3 + c] / 255) * 2 - 1);
          n0[0] -= tx * crack * 0.8;
          n0[1] += ty * crack * 0.8;
          const l = Math.hypot(n0[0], n0[1], n0[2]) || 1;
          for (let c = 0; c < 3; c++) maps.normal[to * 3 + c] = Math.round(((n0[c] / l) * 0.5 + 0.5) * 255);
        }
        maps.arm[to * 3] = Math.round(occlusion[from] * (1 - 0.6 * crack));
        maps.arm[to * 3 + 1] = rough[from];
        maps.arm[to * 3 + 2] = 0;
      }
    }
    console.log(`${id}: cell ${col},${row}`);
  }
  save(maps.colour, 3, size, size, join(out, "color.jpg"));
  save(maps.normal, 3, size, size, join(out, "normal.jpg"));
  save(maps.arm, 3, size, size, join(out, "arm.jpg"));
  execFileSync("rm", ["-r", tmp]);
  const links = ids.map((id) => `[${id}](https://ambientcg.com/a/${id})`).join(", ");
  record(
    name,
    `ambientCG ${links}, CC0, each cut out and drawn round in a cell of ${columns} by ${columns}, greyed ${Math.round(weather * 100)}%${checks ? `, ${checks} checks cut in each` : ""} by \`textures.ts ends\`.`,
  );
}

/** How much of the unit square a polygon covers. */
function area(points: [number, number][]): number {
  let sum = 0;
  points.forEach(([x, y], i) => {
    const [nx, ny] = points[(i + 1) % points.length];
    sum += x * ny - nx * y;
  });
  return Math.abs(sum) / 2;
}

/** The share of a detail texture's side its high pass keeps: what is finer than about this
 * is its grain, what is broader is the surface's own (D362). */
export const DETAIL_PASS = 0.008;
/** How far a detail texture's colour wanders from mid grey, and its normal's slopes from
 * flat, on average (a standard deviation, 0 to 1): its pattern's `strength` scales both. */
export const DETAIL_COLOUR = 0.07;
export const DETAIL_SLOPE = 0.16;

/** A picture's channels with their broad swells taken out, as a high pass does: each texel
 * less the blur round it, the blur wrapping as the texture tiles. */
function highPass(image: string, size: number, channels: 1 | 3): Float32Array {
  const sigma = Math.max(1, size * DETAIL_PASS);
  const raw = (blur: boolean) => {
    const args = [image, "-resize", `${size}x${size}!`];
    if (blur) args.push("-virtual-pixel", "tile", "-blur", `0x${sigma}`);
    if (channels === 1) args.push("-colorspace", "gray");
    return execFileSync(MAGICK,[...args, "-depth", "8", channels === 1 ? "gray:-" : "rgb:-"], { maxBuffer: 1 << 28 });
  };
  const [sharp, soft] = [raw(false), raw(true)];
  const out = new Float32Array(sharp.length);
  for (let i = 0; i < sharp.length; i++) out[i] = (sharp[i] - soft[i]) / 255;
  return out;
}

/** How far values wander from nought: their root mean square. */
export function spread(values: Float32Array, stride = 1, offset = 0): number {
  let sum = 0;
  let n = 0;
  for (let i = offset; i < values.length; i += stride) {
    sum += values[i] * values[i];
    n++;
  }
  return Math.sqrt(sum / Math.max(1, n));
}

/** **detail**: a detail texture from a material already fetched (D362), the fine grain of
 * wood or stone that a construction material's pattern lays over every material of it near:
 * its colour's light and dark with the broad swells taken out, grey about the middle, and
 * its normal map's slopes likewise, each brought to the same spread (`DETAIL_COLOUR`,
 * `DETAIL_SLOPE`) whatever the scan, so a pattern's `strength` means the same for any; no
 * height, a plain ARM map. It tiles as its source does. */
function detail(name: string, source: string, size: number) {
  const from = join(ROOT, "assets/materials", source);
  for (const map of ["color.jpg", "normal.jpg"]) {
    if (!existsSync(join(from, map))) throw new Error(`${source} has no ${map}: fetch it first`);
  }
  const out = join(ROOT, "assets/materials", name);
  mkdirSync(out, { recursive: true });
  const light = highPass(join(from, "color.jpg"), size, 1);
  const gain = DETAIL_COLOUR / Math.max(1e-4, spread(light));
  const colour = Buffer.alloc(size * size * 3);
  for (let i = 0; i < light.length; i++) {
    const v = Math.round((0.5 + light[i] * gain) * 255);
    colour.fill(Math.min(255, Math.max(0, v)), i * 3, i * 3 + 3);
  }
  save(colour, 3, size, size, join(out, "color.jpg"));
  const slopes = highPass(join(from, "normal.jpg"), size, 3);
  const k = DETAIL_SLOPE / Math.max(1e-4, (spread(slopes, 3, 0) + spread(slopes, 3, 1)) / 2);
  const normal = Buffer.alloc(size * size * 3);
  for (let i = 0; i < size * size; i++) {
    let [x, y] = [slopes[i * 3] * 2 * k, slopes[i * 3 + 1] * 2 * k];
    const long = Math.hypot(x, y);
    if (long > 0.95) [x, y] = [(x / long) * 0.95, (y / long) * 0.95];
    const z = Math.sqrt(Math.max(0, 1 - x * x - y * y));
    normal[i * 3] = Math.round((x * 0.5 + 0.5) * 255);
    normal[i * 3 + 1] = Math.round((y * 0.5 + 0.5) * 255);
    normal[i * 3 + 2] = Math.round((z * 0.5 + 0.5) * 255);
  }
  save(normal, 3, size, size, join(out, "normal.jpg"));
  const arm = Buffer.alloc(16 * 16 * 3);
  for (let i = 0; i < 256; i++) arm.set([255, 217, 0], i * 3);
  save(arm, 3, 16, 16, join(out, "arm.jpg"));
  console.log(`detail: ${name} from ${source}, ${size} px, colour x${gain.toFixed(2)}, slopes x${k.toFixed(2)}`);
}

async function main() {
  const { positionals, values } = parseArgs({
    allowPositionals: true,
    options: {
      tile: { type: "string", default: "" },
      size: { type: "string", default: "2048" },
      strength: { type: "string", default: "2" },
      roughness: { type: "string", default: "0.85" },
      name: { type: "string" },
      occlusion: { type: "string", default: "0.6" },
      height: { type: "boolean", default: false },
      columns: { type: "string", default: "4" },
      cut: { type: "string", default: "0.45" },
      around: { type: "string", default: "1" },
      along: { type: "string", default: "2" },
      cover: { type: "string", default: "0.5" },
      seed: { type: "string", default: "1" },
      weather: { type: "string", default: "0.4" },
      saturation: { type: "string", default: "1" },
      lighten: { type: "string", default: "1" },
      exposed: { type: "boolean", default: false },
      soft: { type: "string", default: "0.01" },
      "wood-roughness": { type: "string", default: "" },
      checks: { type: "string", default: "0" },
      help: { type: "boolean", short: "h" },
    },
  });
  const [step, name, ...rest] = positionals;
  if (step === "fetch" && name?.startsWith("ambientcg:")) {
    const id = name.slice("ambientcg:".length);
    await fetchAmbientCg(id, values.name ?? id.toLowerCase(), values.size === "2048" ? "2k" : values.size!);
  } else if (step === "fetch" && name) {
    await fetchMaterial(name, values.name ?? name, values.size === "2048" ? "2k" : values.size!);
  } else if (step === "stack" && name && rest.length) {
    stack(name, rest, values.size === "2048" ? 1024 : Number(values.size), Number(values.occlusion), values.height!);
  } else if (step === "cards" && name && rest.length) {
    await cards(name, rest, Number(values.size), Number(values.columns), Number(values.cut));
  } else if (step === "blend" && name && rest.length === 2) {
    blend(name, rest[0], rest[1], {
      around: Number(values.around),
      along: Number(values.along),
      cover: Number(values.cover),
      size: Number(values.size),
      seed: Number(values.seed),
      saturation: Number(values.saturation),
      lighten: Number(values.lighten),
      exposed: Boolean(values.exposed),
      soft: Number(values.soft),
      woodRoughness: values["wood-roughness"] ? Number(values["wood-roughness"]) : undefined,
    });
  } else if (step === "ends" && name && rest.length) {
    await ends(name, rest, Number(values.size), Number(values.weather), Number(values.checks));
  } else if (step === "detail" && name && rest.length === 1) {
    detail(name, rest[0], Number(values.size));
    record(name, `detail texture (D362): the fine grain of \`${rest[0]}\` (${credited(rest[0])?.replace(/\.$/, "") ?? "see its line"}), its broad swells taken out, its colour grey about the middle, by \`textures.ts detail\`.`);
  } else if (step === "material" && name && rest.length === 1) {
    material(name, resolve(rest[0]), values.tile!, Number(values.size), Number(values.strength), Number(values.roughness));
    record(name, `made from a generated image by \`textures.ts material\`.`);
  } else {
    console.log(`usage:
  node tools/textures/textures.ts fetch <polyhaven id | ambientcg:Id> [--name n] [--size 1k|2k|4k]
  node tools/textures/textures.ts material <name> <image> [--tile y|x|xy] [--size px] [--strength s] [--roughness r]
  node tools/textures/textures.ts stack <name> <material…> [--size px] [--occlusion k] [--height]
  node tools/textures/textures.ts cards <name> <card>=<image | recipe.leaves.json | keep>… [--size px] [--columns n] [--cut a]
  node tools/textures/textures.ts blend <name> <base>=<metres> <over>=<metres> [--around m] [--along m] [--cover f] [--seed n] [--saturation f] [--lighten f] [--exposed] [--soft f] [--wood-roughness r]
  node tools/textures/textures.ts ends <name> <ambientCG TreeEnd id…> [--size px] [--weather f] [--checks n]
  node tools/textures/textures.ts detail <name> <material> [--size px]`);
    process.exit(values.help ? 0 : 2);
  }
}

if (import.meta.main) await main();
