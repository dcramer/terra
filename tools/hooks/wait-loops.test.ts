// The wait-loop hook on sample commands: the hanging waits are refused, other commands pass.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { refusal } from "./wait-loops.ts";

const refused = [
  `until ! pgrep -f "xtask bench" >/dev/null; do sleep 10; done`,
  `while pgrep -f terra_sandbox > /dev/null; do sleep 5; done; echo done`,
  `until ! pgrep terra_sandbox; do :; done`,
  `cd x && until [ -f out/done.txt ]; do sleep 10; done`,
  `while [ ! -s run.log ]; do\n  sleep 2\ndone`,
  `until grep -q finished run.log; do sleep 30; done && tail run.log`,
  `timeout 600 bash -c 'until ! pgrep -f "xtask bench"; do sleep 10; done'`,
  `for i in 1 2; do echo a; done; while true; do pgrep -f x || break; sleep 3; done`,
  `while true; do for f in *.log; do echo $f; done; sleep 5; done`,
];
const pgrepPid = [
  `tail --pid=$(pgrep -f "xtask bench" | head -1) -f /dev/null`,
  "tail --pid=`pgrep -f 'xtask bench' | head -1` -f /dev/null",
  `tail --pid="$(pgrep -f terra_sandbox | head -1)" -f /dev/null`,
  `P=$(pgrep -f "xtask bench" | head -1); tail --pid=$P -f /dev/null`,
  `P=$(pgrep -f "xtask bench" | head -1)\ntail --pid="\${P}" -f /dev/null`,
  "PID=`pgrep -f x`; timeout 600 tail --pid=$PID -f /dev/null",
];
const passed = [
  "tail --pid=12345 -f /dev/null",
  "tail --pid=$! -f /dev/null",
  "P=$!; tail --pid=$P -f /dev/null",
  "P=$(pgrep -f x | head -1); echo $P",
  `echo "tail --pid=$(pgrep -f x) is refused"`,
  "pgrep -f terra_sandbox | head -1",
  "cargo xtask gpu still -- target/release/terra_sandbox --offscreen &\ntail --pid=$! -f /dev/null",
  "sleep 2",
  "sleep 2 && echo ready",
  "pgrep -f terra_sandbox",
  "for f in *.png; do echo $f; done",
  "while read -r line; do echo $line; done < list.txt",
  "for i in 1 2 3; do sleep 1; done",
  "until [ -f x ]; do echo no; break; done",
  "for f in a b; do while read -r l; do echo $l; done < $f; done; sleep 2",
  `git commit -m "Waits: refuse until ... sleep loops and while pgrep -f loops"`,
  `git commit -m "$(cat <<'MSG'\nWaits no longer use\nwhile pgrep -f x; do sleep 5; done\nMSG\n)"`,
  `echo "until ! pgrep -f x; do sleep 10; done"`,
  "ls tools | while read d; do echo $d; done",
];

test("hanging and polling waits are refused, with the way out", () => {
  for (const command of refused) {
    const why = refusal(command);
    assert.match(why, /^Refused: .*tail --pid=<pid> -f \/dev\/null/, command);
    assert.equal(why.split("\n").length, 1, command);
  }
});

test("tail --pid from pgrep is refused, naming the alternatives", () => {
  for (const command of pgrepPid) {
    const why = refusal(command);
    assert.match(why, /^Refused: .*own pid/, command);
    assert.match(why, /`\$!`/, command);
    assert.match(why, /completion notice/, command);
    assert.match(why, /cargo xtask gpu --status/, command);
    assert.equal(why.split("\n").length, 1, command);
  }
});

test("pgrep loops say why they never end", () => {
  assert.match(refusal(refused[0]), /matches the loop's own command line/);
  assert.match(refusal(refused[3]), /polling loop/);
});

test("waiting by pid, plain sleeps and ordinary loops pass", () => {
  for (const command of passed) assert.equal(refusal(command), "", command);
});

test("as a hook: exit 2 with the reason on stderr, exit 0 and silence otherwise", () => {
  const script = new URL("./wait-loops.ts", import.meta.url).pathname;
  const run = (command: string) =>
    spawnSync("node", [script], {
      input: JSON.stringify({
        hook_event_name: "PreToolUse",
        tool_name: "Bash",
        tool_input: { command },
      }),
      encoding: "utf8",
    });
  const no = run(refused[0]);
  assert.equal(no.status, 2);
  assert.match(no.stderr, /^Refused: /);
  assert.equal(no.stdout, "");
  const yes = run("tail --pid=1 -f /dev/null");
  assert.equal(yes.status, 0);
  assert.equal(yes.stderr, "");
  assert.equal(spawnSync("node", [script], { input: "not json" }).status, 0);
});
