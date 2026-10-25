// node --test tools/sandbox/frametimes.test.ts
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { bars, parse, quantile, run } from "./frametimes.ts";

const HEAD = "frame,ms,game_ms,render_ms,gpu_ms,render_busy_ms,render_wait_ms";

/// A run of 100 frames, render-bound at 10 ms with the game at `game`, and extra lines.
function csv(game: number, extra: string[] = []): string {
  const rows = Array.from({ length: 100 - extra.length }, (_, i) => `${i},10,${game},9,8,9.5,0.5`);
  return [HEAD, ...rows, ...extra].join("\n");
}

test("reads the columns by name and takes the nearest rank", () => {
  const frames = parse(`${HEAD}\n7,12.5,4.1,9,8,9.7,0.3\n`);
  assert.deepEqual(frames, [{ frame: 7, ms: 12.5, game: 4.1, busy: 9.7, gpu: 8 }]);
  assert.equal(quantile([1, 2, 3, 4], 0.5), 2);
  assert.equal(quantile(Array.from({ length: 100 }, (_, i) => i + 1), 0.99), 99);
});

test("the same builds pass; a slower game or a long frame in the game fails", () => {
  const old = [5, 5.1, 5.05].map((g, i) => run(`old-${i}`, parse(csv(g))));
  const same = [5.05, 5.1, 5].map((g, i) => run(`new-${i}`, parse(csv(g))));
  assert.ok(bars(old, same).every((b) => b.pass));

  const slower = [1, 2, 3].map((i) => run(`new-${i}`, parse(csv(5.2))));
  assert.deepEqual(bars(old, slower).map((b) => b.pass), [false, true, true, true]);

  // A 30 ms frame long on the GPU is the view's; one long in the game is not.
  const gpuOld = [run("old-1", parse(csv(5, ["99,30,5,9,29,9.5,0.5"]))), ...old.slice(1)];
  const gpu = [run("new-1", parse(csv(5, ["99,30,5,9,29,9.5,0.5"]))), ...same.slice(1)];
  assert.equal(bars(gpuOld, gpu)[2].pass, true);
  const inGame = [run("new-1", parse(csv(5, ["99,30,28,9,8,27,0.5"]))), ...same.slice(1)];
  assert.equal(bars(old, inGame)[1].pass, false);
});

test("a new long frame passes beside an old one on the same ground, fails where old has none", () => {
  const old = [run("old-1", parse(csv(5, ["99,40,5,9,39,9.5,0"]))), run("old-2", parse(csv(5)))];
  const near = [run("new-1", parse(csv(5, ["60,40,5,9,39,9.5,0"])))];
  assert.equal(bars(old, near)[2].pass, true);
  const far = [run("new-1", parse(csv(5, ["20,40,5,9,39,9.5,0"])))];
  assert.equal(bars(old, far)[2].pass, false);
});

test("frame p99 passes inside or under old's range, fails over it", () => {
  const old = [run("old-1", parse(csv(5))), run("old-2", parse(csv(5, ["98,14,5,9,8,9.5,0", "99,14,5,9,8,9.5,0"])))];
  const over = [1, 2].map((i) => run(`new-${i}`, parse(csv(5, ["98,15,5,9,8,9.5,0", "99,15,5,9,8,9.5,0"]))));
  assert.equal(bars(old, over)[3].pass, false);
  assert.equal(bars(old, [run("new-1", parse(csv(5)))])[3].pass, true);
});
