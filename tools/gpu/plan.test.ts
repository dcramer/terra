// The GPU scheduler's admission and order on made-up queues whose answers are known by hand.
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  admit,
  available,
  CLASSES,
  descendants,
  estimate,
  expectedStarts,
  MARGIN_MB,
  order,
  parseApps,
  parseCard,
  parseLocks,
  retryable,
  type Class,
  type Job,
  type Run,
} from "./plan.ts";

const MIN = 60000;
const NOW = 1_000_000_000_000;
/// A 10 GB card with the desktop's 600 MB on it.
const CARD = { totalMB: 10240, otherMB: 600 };

let next = 0;
function job(cls: Class, waitedMin = 0, vramMB = CLASSES[cls].vramMB, more: Partial<Job> = {}): Job {
  return { id: `${cls}-${next++}`, class: cls, vramMB, enqueued: NOW - waitedMin * MIN, ...more };
}
const running = (cls: Class, vramMB: number, usedMB = 0) => job(cls, 0, vramMB, { started: NOW - MIN, usedMB });
const ids = (jobs: Job[]) => jobs.map((j) => j.class);

test("two runs that fit the card start side by side", () => {
  const started = admit([job("film", 0, 6000), job("still", 0, 2500)], [], CARD, NOW);
  assert.deepEqual(ids(started).sort(), ["film", "still"]);
});

test("a still is offered the card before a film that came first", () => {
  const film = job("film", 2);
  const still = job("still", 0);
  assert.deepEqual(ids(order([film, still], NOW)), ["still", "film"]);
  // Only one fits beside a running film of 5 GB: the still.
  assert.deepEqual(ids(admit([film, still], [running("film", 5000)], CARD, NOW)), ["still"]);
});

test("waiting raises a film above new stills, so it does not starve", () => {
  const film = job("film", 20);
  assert.deepEqual(ids(order([job("still"), job("sheet"), film], NOW)), ["film", "still", "sheet"]);
});

test("a film that does not fit holds back smaller jobs only once it has waited its turn", () => {
  const busy = [running("film", 7000)];
  // Waited 2 minutes: the still behind it fills the gap.
  assert.deepEqual(ids(admit([job("film", 2), job("still", 0, 1500)], busy, CARD, NOW)), ["still"]);
  // Waited 16 minutes: it is first and holds the card for itself, so nothing starts.
  assert.deepEqual(ids(admit([job("film", 16), job("still", 0, 1500)], busy, CARD, NOW)), []);
});

test("free memory counts what a running job will grow to, not only what it holds", () => {
  // A film that started a second ago holds 300 MB but asked for 7700.
  const film = running("film", 7700, 300);
  assert.equal(available(CARD, [film]), 10240 - 600 - MARGIN_MB - 7700);
  assert.deepEqual(admit([job("still", 0, 3000)], [film], CARD, NOW), []);
  // One grown past its request is counted at what it holds.
  assert.equal(available(CARD, [running("still", 2000, 2600)]), 10240 - 600 - MARGIN_MB - 2600);
});

test("a benchmark starts only on an empty card, and nothing starts beside it", () => {
  const bench = job("bench", 1);
  assert.deepEqual(ids(admit([bench, job("still")], [running("still", 2000)], CARD, NOW)), ["still"]);
  assert.deepEqual(ids(admit([bench], [], CARD, NOW)), ["bench"]);
  // First in the queue on an empty card, it starts and the still behind it waits.
  assert.deepEqual(ids(admit([job("bench", 11), job("still")], [], CARD, NOW)), ["bench"]);
  assert.deepEqual(admit([job("still")], [running("bench", 8000)], CARD, NOW), []);
  // Once it has waited past its drain time, stills stop starting so the card empties for it.
  assert.deepEqual(admit([job("bench", 6), job("still")], [running("still", 2000)], CARD, NOW), []);
});

test("no more than the most jobs run at once, however small", () => {
  const small = [1, 2, 3, 4, 5].map(() => job("still", 0, 500));
  assert.equal(admit(small, [], CARD, NOW, 4).length, 4);
});

test("the expected wait plays the queue forward as running jobs end", () => {
  const film = running("film", 7000);
  const still = job("still", 0, 3000);
  const starts = expectedStarts([still], [film], CARD, NOW, (j) => (j.class === "film" ? 10 : 1));
  // The film started a minute ago and runs ten: the still starts in nine.
  assert.equal(starts.get(still.id), NOW + 9 * MIN);
  const huge = job("film", 0, 20000);
  assert.equal(expectedStarts([huge], [], CARD, NOW, () => 1).get(huge.id), Infinity);
});

test("estimates are learned from measured peaks once three are known", () => {
  const runs = (peaks: number[]): Run[] => peaks.map((peakMB) => ({ class: "still", peakMB, runS: 30, code: 0 }));
  assert.equal(estimate("still", runs([1000, 1200])), CLASSES.still.vramMB);
  assert.equal(estimate("still", runs([1000, 1200, 1400])), 1600);
  assert.equal(estimate("film", runs([1000, 1200, 1400])), CLASSES.film.vramMB);
});

test("a lost device and running out of memory are worth a retry, other failures not", () => {
  assert.match(retryable("wgpu error: Validation Error\nParent device is lost") ?? "", /lost/);
  assert.match(retryable("Error in Queue::submit: OutOfMemory") ?? "", /memory/);
  assert.equal(retryable("thread 'main' panicked at src/main.rs:3:5"), null);
});

test("nvidia-smi's card and per-process lines are read", () => {
  assert.deepEqual(parseCard("10240, 944\n"), { totalMB: 10240, usedMB: 944 });
  assert.equal(parseCard("No devices were found"), null);
  assert.deepEqual([...parseApps("1996, 12\n3166, 122\n")], [[1996, 12], [3166, 122]]);
});

test("the holders and waiters of a lock are read from /proc/locks", () => {
  const locks = "33: FLOCK  ADVISORY  READ  704977 00:1c:7468718 0 EOF\n" +
    "33: -> FLOCK  ADVISORY  WRITE 766125 00:1c:7468718 0 EOF\n" +
    "34: POSIX  ADVISORY  WRITE 1200 103:02:555 0 EOF\n";
  assert.deepEqual(parseLocks(locks, 7468718), [
    { pid: 704977, write: false, waiting: false },
    { pid: 766125, write: true, waiting: true },
  ]);
});

test("a run's processes are found under it", () => {
  const parents = new Map([[10, 1], [11, 10], [12, 11], [20, 1]]);
  assert.deepEqual([...descendants(10, parents)].sort(), [10, 11, 12]);
});
