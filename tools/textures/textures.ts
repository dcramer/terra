// Makes the game's materials (D14, docs/art-style.md): fetches a scanned one from a free
// library, makes one from a generated image no library has, and stacks several into one
// texture array for a surface that blends them, such as the ground.
//
//   node tools/textures/textures.ts fetch <polyhaven id> [--name n] [--size 1k|2k|4k]
//   node tools/textures/textures.ts material <name> <image> [--tile y|x|xy] [--size px]
//                                            [--strength s] [--roughness r]
//   node tools/textures/textures.ts stack <name> <material…> [--size px] [--occlusion k]
//
// Each writes assets/materials/<name>/, and adds or updates its line in that folder's
// README.md, which says where every material came from.
//
// **fetch** downloads a Poly Haven material (CC0) as color.jpg, normal.jpg (OpenGL's
// convention, as Bevy reads it), arm.jpg (occlusion, roughness and metal) and, where it
// has one, height.jpg for parallax.
//
// **stack** puts the color and normal maps of several materials one above the next, each
// --size (1024) square, for Bevy to load as an array (`ImageArrayLayout::RowCount`). Its
// occlusion is multiplied into the colour, by --occlusion (0.6), since a stack has no
// ARM map of its own.
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
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";

const ROOT = resolve(import.meta.dirname, "..", "..");

/** An image's lightness, a byte a pixel, and its size. */
function grey(image: string, width?: number): { data: Buffer; width: number; height: number } {
  const resize = width ? ["-resize", `${width}x`] : [];
  const size = execFileSync("convert", [image, ...resize, "-format", "%w %h", "info:"]).toString().split(" ");
  const data = execFileSync("convert", [image, ...resize, "-colorspace", "gray", "-depth", "8", "gray:-"], {
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
  const full = execFileSync("convert", [image, "-format", "%w %h", "info:"]).toString().split(" ").map(Number);
  let [width, height] = full;
  if (tile.includes("x")) width = Math.round((period(sample.data, sample.width, sample.height, "x") * full[0]) / sample.width);
  if (tile.includes("y")) height = Math.round((period(sample.data, sample.width, sample.height, "y") * full[1]) / sample.height);
  console.log(`${name}: repeats every ${width} by ${height} of ${full[0]} by ${full[1]}`);
  const color = join(out, "color.jpg");
  execFileSync("convert", [image, "-crop", `${width}x${height}+0+0`, "+repage", "-resize", `${size}x`, "-quality", "92", color]);
  // Height from lightness, softened; its slopes across and down make the normal's red and
  // green, the way up its blue (OpenGL's convention, as glTF and Bevy use).
  const slope = (kernel: string) => [
    "(", color, "-colorspace", "gray", "-blur", "0x1.5", "-define", "convolve:scale=" + strength / 8,
    "-bias", "50%", "-morphology", "Convolve", kernel, ")",
  ];
  execFileSync("convert", [
    ...slope("3x3: -1,0,1 -2,0,2 -1,0,1"),
    ...slope("3x3: 1,2,1 0,0,0 -1,-2,-1"),
    "(", color, "-fill", "white", "-colorize", "100", ")",
    "-combine", "-quality", "92", join(out, "normal.jpg"),
  ]);
  // Its height, for parallax: how deep each part reads, lighter higher.
  execFileSync("convert", [color, "-colorspace", "gray", "-blur", "0x2", "-normalize", "-quality", "92", join(out, "height.jpg")]);
  const green = Math.round(roughness * 255);
  execFileSync("convert", ["-size", "16x16", `xc:rgb(255,${green},0)`, join(out, "arm.jpg")]);
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
    execFileSync("convert", [join(out, "height.png"), "-colorspace", "gray", "-depth", "8", "-quality", "92", join(out, "height.jpg")]);
    execFileSync("rm", [join(out, "height.png")]);
  }
  const metres = info.dimensions ? ` ${(info.dimensions[0] / 1000).toFixed(2)} m across.` : "";
  record(name, `Poly Haven [${info.name ?? id}](https://polyhaven.com/a/${id}), CC0, ${size}.${metres}`);
  console.log(`${name}: ${out}${metres}`);
}

function stack(name: string, materials: string[], size: number, occlusion: number) {
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
  execFileSync("convert", [...colours, "-append", "-quality", "92", join(out, "color.jpg")]);
  const normals = materials.flatMap((m) => ["(", join(dir(m), "normal.jpg"), ...square, ")"]);
  execFileSync("convert", [...normals, "-append", "-quality", "92", join(out, "normal.jpg")]);
  const layers = materials.map((m) => `\`${m}\` (${credited(m)?.replace(/\.$/, "") ?? "uncredited"})`);
  record(name, `a stack, in this order, ${size} pixels each, of ${layers.join("; ")}.`);
  console.log(`${name}: ${materials.length} layers in ${out}`);
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
      help: { type: "boolean", short: "h" },
    },
  });
  const [step, name, ...rest] = positionals;
  if (step === "fetch" && name) {
    await fetchMaterial(name, values.name ?? name, values.size === "2048" ? "2k" : values.size!);
  } else if (step === "stack" && name && rest.length) {
    stack(name, rest, values.size === "2048" ? 1024 : Number(values.size), Number(values.occlusion));
  } else if (step === "material" && name && rest.length === 1) {
    material(name, resolve(rest[0]), values.tile!, Number(values.size), Number(values.strength), Number(values.roughness));
    record(name, `made from a generated image by \`textures.ts material\`.`);
  } else {
    console.log(`usage:
  node tools/textures/textures.ts fetch <polyhaven id> [--name n] [--size 1k|2k|4k]
  node tools/textures/textures.ts material <name> <image> [--tile y|x|xy] [--size px] [--strength s] [--roughness r]
  node tools/textures/textures.ts stack <name> <material…> [--size px] [--occlusion k]`);
    process.exit(values.help ? 0 : 2);
  }
}

if (import.meta.main) await main();
