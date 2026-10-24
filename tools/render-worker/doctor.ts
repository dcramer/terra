// `box.ts doctor` (docs/render-worker.md, D445): what the render box needs, a line each, and
// for what is wrong the fix; the client runs it over ssh (`cargo xtask remote-render --doctor`).

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, statfsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { type Box, boxEnv, daemonInfo, git, listJobs } from "./box.ts";
import { ago, boxSettings, gpuFix } from "./jobs.ts";

const sh = (cmd: string, args: string[]) => {
  const r = spawnSync(cmd, args, { encoding: "utf8" });
  return { ok: r.status === 0, out: `${r.stdout ?? ""}${r.stderr ?? ""}`.trim() };
};

export function doctor(b: Box): number {
  let bad = 0;
  const line = (ok: boolean | "warn", text: string, fix: string[] = []) => {
    if (ok === false) bad++;
    console.log(`${ok === true ? "✓" : ok === "warn" ? "!" : "✗"} ${text}`);
    for (const f of ok === true ? [] : fix) console.log(`    ${f}`);
  };

  const kernel = sh("uname", ["-r"]).out;
  const smi = sh("nvidia-smi", ["--query-gpu=name,memory.total,driver_version", "--format=csv,noheader"]);
  if (smi.ok) line(true, `GPU: ${smi.out.split("\n")[0]}`);
  else {
    const pkgs = sh("dpkg-query", ["-W", "-f=${Package} ${Status}\\n", "nvidia-driver-*"]).out;
    const branch = /nvidia-driver-(\d+)(?:-open)?\s+install ok installed/.exec(pkgs)?.[1];
    line(false, `GPU: nvidia-smi fails (${smi.out.split("\n")[0] || "not found"})`, gpuFix(kernel, branch, sh("modinfo", ["-n", "nvidia"]).ok));
  }

  const icds = ["/usr/share/vulkan/icd.d", "/etc/vulkan/icd.d"].flatMap((d) => (existsSync(d) ? readdirSync(d) : []));
  const loader = sh("sh", ["-c", "ldconfig -p | grep -q 'libvulkan.so.1 '"]).ok;
  const info = sh("vulkaninfo", ["--summary"]);
  const nvDevice = /deviceName\s*=\s*.*NVIDIA/i.exec(info.out);
  if (!loader || !icds.includes("nvidia_icd.json")) line(false, `Vulkan: ${loader ? "" : "no loader (libvulkan1) "}${icds.includes("nvidia_icd.json") ? "" : "no NVIDIA ICD"}`.trim(), ["the NVIDIA driver package brings its ICD; `sudo apt-get install -y libvulkan1`"]);
  else if (info.ok) line(nvDevice ? true : false, `Vulkan: ${nvDevice ? nvDevice[0].replace(/\s+/g, " ") : "vulkaninfo sees no NVIDIA device"}`);
  else line(true, "Vulkan: the loader and the NVIDIA ICD are installed (vulkan-tools is not, so the device is not listed)");

  line(true, `node: ${process.version} (${process.execPath})`);
  const node22 = Number(process.versions.node.split(".")[0]) >= 22;
  if (!node22) line(false, "node is older than 22.18", ["cargo xtask remote-render --update installs Node 22 into the worker's folder"]);

  const cargo = sh("cargo", ["--version"]);
  line(cargo.ok, `cargo: ${cargo.ok ? cargo.out : "not on PATH"}`, ["rustup: https://rustup.rs (no sudo)"]);
  const libs = [
    ["alsa", "libasound2-dev"],
    ["libudev", "libudev-dev"],
    ["wayland-client", "libwayland-dev"],
    ["xkbcommon", "libxkbcommon-dev"],
  ].filter(([pc]) => !sh("pkg-config", ["--exists", pc]).ok);
  line(!libs.length, libs.length ? `build libraries missing: ${libs.map(([, p]) => p).join(", ")} (the sandbox does not build)` : "build libraries: alsa, udev, wayland, xkbcommon", [
    `sudo apt-get install -y pkg-config ${libs.map(([, p]) => p).join(" ")}`,
  ]);
  const ffmpeg = sh("ffmpeg", ["-version"]).ok;
  line(ffmpeg || "warn", ffmpeg ? "ffmpeg" : "ffmpeg: not installed; a film job cannot encode its clip", ["sudo apt-get install -y ffmpeg"]);

  const fs = statfsSync(existsSync(b.dir) ? b.dir : homedir());
  const freeGb = Math.round((fs.bavail * fs.bsize) / 1024 ** 3);
  line(freeGb >= 100 || (freeGb >= 30 ? "warn" : false), `disk: ${freeGb} GB free for ${b.dir}`, ["each slot's build and caches take tens of GB; free some, lower TERRA_RENDER_SLOTS, or old jobs go after 7 days"]);

  const s = boxSettings(boxEnv(b));
  line(!("error" in s), "error" in s ? `settings: ${s.error}` : `settings: ${s.policy.window ? `jobs start ${boxEnv(b).TERRA_RENDER_WINDOW}` : "jobs start at any hour"}${s.ciLock ? `, after the CI lock ${s.ciLock}` : ""}, ${s.slots.slots} slots (${s.slots.gpuSlots} for GPU jobs)`, [
    `fix ${join(b.repo, ".env")} (.env.example)`,
  ]);

  const rev = git(b.repo, ["rev-parse", "--short=9", "HEAD"]).stdout.trim();
  const d = daemonInfo(b);
  const now = new Date();
  if (!d) line(false, "daemon: never started", ["cargo xtask remote-render --update"]);
  else if (!d.alive) line(false, `daemon: not running (last word ${ago(d.beat, now)} ago: ${d.status})`, ["cargo xtask remote-render --update starts it; --logs daemon shows why it stopped"]);
  else line(d.rev === rev || "warn", `daemon: ${d.status}, ${d.mode}, at ${d.rev}${d.rev === rev ? "" : ` (the clone is at ${rev}; it restarts after its jobs)`}`);
  const jobs = listJobs(b);
  const queued = jobs.filter((j) => j.state === "queued").length;
  const lastWarm = jobs.filter((j) => j.kind === "warm" && j.state !== "cancelled").at(-1);
  const warms = jobs.filter((j) => j.kind === "warm" && j.rev === lastWarm?.rev && j.state !== "cancelled");
  const count = (st: string) => warms.filter((j) => j.state === st).length;
  line(true, `jobs: ${queued} queued, ${jobs.length} kept${warms.length ? `; last update's warm builds: ${count("done")} done, ${count("running") + count("queued")} to go, ${count("failed")} failed` : ""}`);
  for (const w of warms.filter((j) => j.state === "failed")) line(false, `the warm build in slot ${w.slot ?? 1} failed: --logs ${w.id}`);
  return bad ? 1 : 0;
}
