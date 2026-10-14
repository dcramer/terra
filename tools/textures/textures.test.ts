import assert from "node:assert/strict";
import { test } from "node:test";
import { ambientCgFile, credit, keyOut, meanColour, outline, period, rim, tileNoise } from "./textures.ts";

test("an ambientCG material's zip is named by its id and size as ambientCG names it", () => {
  assert.equal(ambientCgFile("Ground104", "2k"), "Ground104_2K-JPG");
});

test("a picture on blue is cut out of it, its edge unmixed from the blue", () => {
  // Three by three: blue all round a green middle, and a pixel half green, half blue.
  const blue = [0, 0, 255, 255];
  const green = [60, 140, 40, 255];
  const half = [30, 70, 148, 255];
  const pixels = [blue, blue, blue, blue, green, half, blue, blue, blue].flat();
  const out = keyOut(Buffer.from(pixels), 3, 3);
  assert.equal(out[4 * 4 + 3], 255);
  assert.equal(out[0 * 4 + 3], 0);
  // The half-covered pixel is partly covered, and reads green, not blue.
  assert.ok(out[5 * 4 + 3] > 64 && out[5 * 4 + 3] < 240, `cover ${out[5 * 4 + 3]}`);
  assert.ok(out[5 * 4 + 2] <= out[5 * 4 + 1], "blue left at the edge");
  // What is not covered takes the colour of what is.
  assert.deepEqual([...out.subarray(0, 3)], [60, 140, 40]);
});

test("a card's octagon holds everything it covers and cuts its corners", () => {
  // A diamond in a 9 by 9 picture: every covered pixel is inside its octagon.
  const size = 9;
  const data = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++)
    for (let x = 0; x < size; x++) if (Math.abs(x - 4) + Math.abs(y - 4) <= 3) data[(y * size + x) * 4 + 3] = 255;
  const shape = outline(data, size, size, 0.5);
  assert.equal(shape.length, 8);
  for (let y = 0; y < size; y++)
    for (let x = 0; x < size; x++) {
      if (!data[(y * size + x) * 4 + 3]) continue;
      const [px, py] = [(x + 0.5) / size, (y + 0.5) / size];
      shape.forEach(([ax, ay], i) => {
        const [bx, by] = shape[(i + 1) % 8];
        assert.ok((bx - ax) * (py - ay) - (by - ay) * (px - ax) >= -1e-6, `pixel ${x},${y} outside`);
      });
    }
  // Black where it covers.
  assert.ok(meanColour(data, 0.5).every((c) => c === 0));
});

test("an image of rows repeats where its rows do", () => {
  // Eight wide and 40 high: a light row every 10 pixels.
  const width = 8;
  const height = 40;
  const data = Buffer.alloc(width * height);
  for (let y = 0; y < height; y++) data.fill(y % 10 === 0 ? 220 : 60, y * width, (y + 1) * width);
  assert.equal(period(data, width, height, "y"), 20);
});

test("a material's credit goes in name order and replaces what it said before", () => {
  const readme = "# Materials\n\n- `a`: one.\n- `c`: three.\n";
  const added = credit(readme, "b", "two.");
  assert.equal(added, "# Materials\n\n- `a`: one.\n- `b`: two.\n- `c`: three.\n");
  assert.equal(credit(added, "b", "again."), "# Materials\n\n- `a`: one.\n- `b`: again.\n- `c`: three.\n");
  assert.equal(credit(readme, "d", "four."), "# Materials\n\n- `a`: one.\n- `c`: three.\n- `d`: four.\n");
});

test("a blend's mask repeats as the texture does, varies within it and stays in 0 to 1", () => {
  const [width, height] = [64, 32];
  const noise = tileNoise(width, height, [4, 2], 7);
  // Smooth across its own seams: each edge's pixels close to the far edge's, as to their
  // neighbours inside.
  const at = (x: number, y: number) => noise[((y + height) % height) * width + ((x + width) % width)];
  for (let y = 0; y < height; y++) assert.ok(Math.abs(at(0, y) - at(-1, y)) < 0.08, `seam across at ${y}`);
  for (let x = 0; x < width; x++) assert.ok(Math.abs(at(x, 0) - at(x, -1)) < 0.08, `seam down at ${x}`);
  const [low, high] = [Math.min(...noise), Math.max(...noise)];
  assert.ok(low >= 0 && high <= 1 && high - low > 0.3, `${low} to ${high}`);
  assert.notDeepEqual(noise, tileNoise(width, height, [4, 2], 8));
});

test("a cut-out's rim is found round it, every degree", () => {
  // A disc of radius 20 in a 64 by 64 picture, off its middle.
  const size = 64;
  const alpha = Buffer.alloc(size * size);
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) if (Math.hypot(x - 30, y - 34) <= 20) alpha[y * size + x] = 255;
  const edge = rim(alpha, size, size, [30, 34]);
  assert.equal(edge.length, 360);
  for (const r of edge) assert.ok(Math.abs(r - 20) < 1.2, `rim at ${r}`);
});

test("the first credit is set apart from the README's introduction", () => {
  assert.equal(credit("# Materials\n\nWhere.\n", "a", "one."), "# Materials\n\nWhere.\n\n- `a`: one.\n");
});
