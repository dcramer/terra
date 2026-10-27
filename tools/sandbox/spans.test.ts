// node --test tools/sandbox/spans.test.ts
import { strict as assert } from "node:assert";
import { test } from "node:test";
import { attribute, costliest, pair, spanName } from "./spans.ts";

const B = (tid: number, ts: number, name: string) => ({ ph: "B", pid: 1, tid, ts, name });
const E = (tid: number, ts: number) => ({ ph: "E", pid: 1, tid, ts });

test("names frames, schedules, systems and their commands, and nothing else", () => {
  assert.equal(spanName('system: name="terra_world::sections::lay<Foo<Bar>>"'), "terra_world::sections::lay");
  assert.equal(spanName('system: name="a::f<M>"', true), "a::f<M>");
  assert.equal(spanName('system: name="<a::P as b::Plugin>::build::{{closure}}"'), "<a::P as b::Plugin>::build::{{closure}}");
  assert.equal(spanName('system_commands: name="a::b"'), "a::b [commands]");
  assert.equal(spanName("schedule: name=PostUpdate"), "[schedule] PostUpdate");
  assert.equal(spanName("main app: "), "main app");
  assert.equal(spanName("sub app: name=RenderApp"), "render app");
  assert.equal(spanName("update: "), null);
});

// The game's frames 0-5 ms and 10-34 ms on the main thread, the first followed by the render
// world's; a worker's spans belong to the frame they start in.
const TRACE = [
  B(0, 0, "main app: "),
  B(0, 50, "schedule: name=PostUpdate"),
  B(0, 100, 'system: name="a"'),
  E(0, 1_100),
  B(9, 2_000, 'system: name="b"'),
  E(9, 4_000),
  E(0, 3_050),
  E(0, 5_000),
  B(0, 5_000, "sub app: name=RenderApp"),
  B(9, 5_200, 'system: name="r"'),
  E(9, 5_700),
  B(0, 5_800, 'system: name="r"'),
  E(0, 5_900),
  E(0, 8_000),
  B(0, 10_000, "main app: "),
  { ph: "X", pid: 1, tid: 9, ts: 10_100, dur: 20_000, name: 'system: name="a"' },
  E(0, 34_000),
];

test("puts each span in the world whose frame it starts in", async () => {
  const trace = await pair(TRACE);
  const game = attribute(trace, "game");
  assert.deepEqual(game.map((f) => f.ms), [5, 24]);
  assert.equal(game[0].systems.get("a"), 1);
  assert.equal(game[0].systems.get("b"), 2);
  assert.equal(game[0].systems.get("[schedule] PostUpdate"), 3);
  assert.equal(game[0].systems.has("r"), false);
  assert.equal(game[1].systems.get("a"), 20);
  const render = attribute(trace, "render");
  assert.deepEqual(render.map((f) => f.ms), [3]);
  assert.equal(render[0].systems.get("r"), 0.6);
  assert.equal(render[0].systems.has("a"), false);
  assert.deepEqual(costliest(game, 1, (n) => !n.startsWith("["))[0], { name: "a", p50: 1, p99: 20, max: 20 });
});
