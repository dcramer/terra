// node --test tools/sandbox/why.test.ts: the function a change's line is in, and the answer's lines.

import assert from "node:assert/strict";
import { test } from "node:test";
import { describeWhy, enclosingFn, type Why } from "./why.ts";

const SOURCE = `fn other() {
    let a = 1;
}

/// A system.
pub(crate) fn warp(
    options: Res<Options>,
    player: Single<&mut Transform>,
) {
    let mut transform = player.into_inner();
    transform.translation = feet;
    app.add_systems(Update, |mut moved: Query<&mut Transform>| {
        for mut t in &mut moved {
            t.translation.x += 1.0;
        }
    });
    fn inner() {}
    done();
}
`;

test("a line is told by the function it is in, through closures and past nested items", () => {
  assert.equal(enclosingFn(SOURCE, 2), "other");
  assert.equal(enclosingFn(SOURCE, 11), "warp");
  assert.equal(enclosingFn(SOURCE, 14), "warp");
  assert.equal(enclosingFn(SOURCE, 18), "warp");
  assert.equal(enclosingFn("let x = 1;\n", 1), undefined);
});

const answer = (tracked: boolean): Why => ({
  entity: 4294967301,
  name: "Player",
  tracked,
  frame: 120,
  kept: 121,
  spawned: { by: tracked ? { file: "src/w.rs", line: 2 } : null, when: { frame: 3, ago: 117 } },
  components: [
    { name: "Transform", type: "bevy_transform::components::transform::Transform", by: tracked ? { file: "src/w.rs", line: 11 } : null, changed: { frame: 120, ago: 0 }, added: { frame: 3, ago: 117 }, immutable: false },
    { name: "ChildOf", type: "bevy_ecs::hierarchy::ChildOf", by: null, changed: { frame: null, ago: null }, added: null, immutable: true },
  ],
});

test("the answer reads as a line a component, naming the function behind each change", () => {
  const read = (file: string) => (file === "src/w.rs" ? SOURCE : undefined);
  assert.deepEqual(describeWhy(answer(true), read), [
    "Player (4294967301) at frame 120: spawned src/w.rs:2 in other, frame 3, 117 frames ago",
    "  Transform  src/w.rs:11 in warp, frame 120, this frame",
    "  ChildOf    (set when inserted; its line is not readable) over 121 frames ago",
  ]);
  const untracked = describeWhy(answer(false), read);
  assert.equal(untracked[1], "  Transform  frame 120, this frame");
  assert.match(untracked.at(-1)!, /built without track_location/);
  // The game's own word on which build keeps the lines is what is printed.
  const told = describeWhy({ ...answer(false), note: "start a dev build: `r start`" }, read);
  assert.equal(told.at(-1), "(start a dev build: `r start`)");
});

test("a body drawn between ticks is told drawn by its drawing and moved by what moved it", () => {
  const read = (file: string) => (file === "src/w.rs" ? SOURCE : undefined);
  const drawn = (tracked: boolean, moved: Why["components"][0]["moved"]): Why => {
    const why = answer(tracked);
    why.components = [{ ...why.components[0], by: tracked ? { file: "src/w.rs", line: 2 } : null, drawn: true, moved }];
    return why;
  };
  const warped = { by: { file: "src/w.rs", line: 17 }, changed: { frame: 100, ago: 20 }, in_tick: false };
  assert.equal(
    describeWhy(drawn(true, warped), read)[1],
    "  Transform  src/w.rs:2 in other, frame 120, this frame; drawn between ticks, last moved outside the ticks by src/w.rs:17 in warp, frame 100, 20 frames ago",
  );
  assert.equal(
    describeWhy(drawn(false, { ...warped, by: null, in_tick: true }), read)[1],
    "  Transform  frame 120, this frame; drawn between ticks, last moved in a tick, frame 100, 20 frames ago",
  );
  assert.match(describeWhy(drawn(true, null), read)[1], /; drawn between ticks, not moved since it was spawned$/);
});
