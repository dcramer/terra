#!/usr/bin/env node
// The portal's census (docs/instances.md "The test that proves nothing leaks", D469): one run
// of `--experiment portal` that crosses to the placeholder realm and back five times, reading
// the census in the forest before the first trip and after each return, and in the realm, and
// holds it to the doc's rules: after trips 2 to 5 the forest's entities by archetype, assets by
// type and resources equal trip 1's exactly, and the GPU's memory and the process's resident
// memory are within 2% of trip 1's; what lives (creatures, D469) is counted apart, by kind, and
// each kind living before the first trip lives after every return, and no kind of entity
// there before the first trip is gone after it; in the realm, what of the
// forest is still there is listed; every crossing is over within 3 s (D501), as the game
// measured it;
// the forest's state before the first trip and after the last differs only in the player's
// place. Says each crossing's time, as the game measured it (step through to play).
//
//   node tools/sandbox/portal.ts [--trips 5] [--settle 5] [--debug] [--systems]
//
// --systems builds with per-system timing and, at the end, says which systems held each
// crossing's long frames (the freeze warnings of shots/remote.log, shortened).
//
// A release build unless --debug. Node 22.18 or later; no dependencies.

import { call, describe, said, start, stop, press, waitSeconds, waitUntil } from "./client.ts";
import * as snaps from "./snap.ts";

const args = process.argv.slice(2);
const value = (flag: string, fallback: number) => {
  const at = args.indexOf(flag);
  return at >= 0 ? Number(args[at + 1]) : fallback;
};
const TRIPS = value("--trips", 5);
// Game seconds let pass in each world before its census, for what streams in round the eye.
const SETTLE_S = value("--settle", 5);
const MEMORY_SLACK = 0.02;
// The longest a crossing's cover may last, s (D501).
const COVER_S = 3;

type Census = {
  entities: Record<string, number>;
  assets: Record<string, number>;
  resources: string[];
  world: { key: number; kind: string; seed: number } | null;
  travel: string | null;
  entered: number | null;
  gpu: { allocated_mib: number; reserved_mib: number } | null;
  resident_mib: number | null;
  /// What lives on by the clock (creatures), by kind, and the entities it is made of: put out
  /// again by its rules on entering, so not compared exactly (D469).
  living: Record<string, number>;
  living_parts: number;
};
const census = async () => (await call("world.census")) as Census;
// What is compared exactly: the counts, without how many worlds the run has entered and
// without what lives, whose numbers move with time.
const counts = (c: Census) => {
  const { entered: _entered, living: _living, living_parts: _parts, ...rest } = snaps.counted(c) as Record<string, unknown>;
  return rest;
};
const living = (c: Census) =>
  Object.entries(c.living ?? {})
    .map(([kind, n]) => `${n} ${kind}`)
    .join(", ") || "none";
// Each kind living in the forest as it was first entered is living there after a return.
const lost = (first: Census, home: Census) => Object.keys(first.living ?? {}).filter((kind) => !(home.living ?? {})[kind]);

const slow: string[] = [];
let entered = 0;
let line = 0;
/// Through the nearest portal, then the census once the next world plays and has settled.
async function cross(): Promise<{ census: Census; took: string }> {
  entered += 1;
  await press("n");
  await waitUntil(`world ${entered} to be played`, census, (c) => c.entered === entered && c.travel === "Playing", 900);
  await waitSeconds(SETTLE_S);
  const heard = await said(line, "Portal: crossed", 4);
  line = heard.next_line;
  const took = heard.lines.map((l) => l.text.replace(/^.*Portal: /, "")).join("; ") || "(no crossing said)";
  const seconds = Number(/ in ([0-9.]+) s/.exec(took)?.[1] ?? NaN);
  // The game not saying how long is a failure too: the bar is held only by what it measured.
  if (!(seconds <= COVER_S)) slow.push(`crossing ${entered} took ${Number.isNaN(seconds) ? "a time the game did not say" : `${seconds} s`}`);
  return { census: await census(), took };
}

const within = (a: number | null | undefined, b: number | null | undefined) => a != null && b != null && Math.abs(b - a) <= MEMORY_SLACK * a;
const memory = (c: Census) => `gpu ${c.gpu?.allocated_mib ?? "?"} MiB allocated, ${c.gpu?.reserved_mib ?? "?"} reserved; resident ${c.resident_mib ?? "?"} MiB`;

await start(["--offscreen", "--mute", "--experiment", "portal"], { release: !args.includes("--debug"), systems: args.includes("--systems") });
let failed = false;
try {
  await waitSeconds(SETTLE_S);
  const before = await snaps.take("before");
  const first = await census();
  line = (await said(0, undefined, 1)).next_line;
  console.log(`forest: ${first.world?.kind} ${first.world?.key}, ${Object.values(first.entities).reduce((a, b) => a + b, 0)} entities, ${memory(first)}; living ${living(first)}`);
  let trip1: Census | undefined;
  for (let trip = 1; trip <= TRIPS; trip++) {
    const there = await cross();
    const realm = there.census;
    console.log(`trip ${trip} out: ${there.took}; ${realm.world?.kind} ${realm.world?.key}, ${memory(realm)}`);
    if (trip === 1) {
      const kept = first.resources.filter((r) => realm.resources.includes(r));
      const gone = first.resources.filter((r) => !realm.resources.includes(r));
      const assets = Object.entries(first.assets).filter(([type, n]) => (realm.assets[type] ?? 0) >= n && n > 0);
      console.log(`  in the realm: ${gone.length} of the forest's ${first.resources.length} resources gone, ${kept.length} present in both; ${assets.length} asset types at least as many as the forest's`);
    }
    const back = await cross();
    const home = back.census;
    console.log(`trip ${trip} back: ${back.took}; ${memory(home)}; living ${living(home)}`);
    if (home.world?.key !== first.world?.key) {
      console.log(`  FAIL: back in ${home.world?.kind} ${home.world?.key}, not the forest`);
      failed = true;
    }
    const gone = lost(first, home);
    if (gone.length) {
      console.log(`  FAIL: ${gone.join(", ")} living before the first trip, not after this one`);
      failed = true;
    }
    if (!trip1) {
      trip1 = home;
      const changed = snaps.changes(counts(first), counts(home));
      console.log(`  against before the first trip (caches may fill): ${changed.length} changes${changed.length ? ": " + changed.slice(0, 12).join("; ") : ""}`);
      // A cache may fill on the first trip; nothing may be gone. A kind of entity there before
      // any world was left and missing after the first return was the app's, taken with the
      // world: the camera that shows the picture went so, the screen black from the first
      // crossing on, and this line had listed it as a change.
      const taken = changed.filter((change) => change.startsWith("- entities."));
      if (taken.length) {
        console.log(`  FAIL: a crossing took what was there before any world was left: ${taken.join("; ").slice(0, 600)}. It is the app's, not the world's: mark it terra_world::instance::Global where it is spawned.`);
        failed = true;
      }
      continue;
    }
    const changed = snaps.changes(counts(trip1), counts(home));
    const steady = within(trip1.gpu?.allocated_mib, home.gpu?.allocated_mib) && within(trip1.resident_mib, home.resident_mib);
    console.log(`  against trip 1: ${changed.length ? `FAIL ${changed.length} changes: ${changed.slice(0, 20).join("; ")}` : "counts equal"}; memory ${steady ? "within 2%" : "FAIL past 2%"}`);
    failed ||= changed.length > 0 || !steady;
  }
  const after = await snaps.take("after");
  const diff = snaps.diff({ ...before, census: undefined }, { ...after, census: undefined }, { most: 30 });
  console.log(`forest's state, before the first trip to after the last: ${diff.join("\n  ")}`);
  if (slow.length > 0) {
    failed = true;
    console.log(`FAIL: the cover lasted over ${COVER_S} s (D501): ${slow.join("; ")}. Which systems hold the long frames: add --systems.`);
  }
  if (args.includes("--systems")) {
    // Each long frame of a crossing, by what held it.
    const { lines } = await said(0, "freeze: frame", 60).catch(() => ({ lines: [] }));
    for (const l of lines) console.log(`  ${describe(l).replace(/^.*freeze: /, "").slice(0, 700)}`);
  }
} catch (error) {
  failed = true;
  console.log(`FAIL: ${error instanceof Error ? error.message : error}`);
  const { lines } = await said(0, "Portal", 40).catch(() => ({ lines: [] }));
  for (const l of lines) console.log(`  ${describe(l)}`);
} finally {
  await stop().catch(() => undefined);
}
console.log(failed ? "portal: FAIL" : "portal: ok");
process.exit(failed ? 1 : 0);
