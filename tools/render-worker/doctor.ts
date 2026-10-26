// `box.ts doctor` (docs/render-worker.md, D445): what the render box needs, a line each, and
// for what is wrong the fix; the client runs it over ssh (`cargo xtask remote-render --doctor`).

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, statfsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { type Box, boxEnv, daemonInfo, git, listJobs } from "./box.ts";
import { ago, boxSettings, displayPlan, gpuFix } from "./jobs.ts";

const sh = (cmd: string, args: string[]) => {
  const r = spawnSync(cmd, args, { encoding: "utf8" });
  return { ok: r.status === 0, out: `${r.stdout ?? ""}${r.stderr ?? ""}`.trim() };
};

type Line = (ok: boolean | "warn", text: string, fix?: string[]) => void;

/// The display jobs draw through (display.ts): winit needs one even offscreen. What the running
/// daemon gives them, else what it would; and the X client libraries winit loads.
function display(b: Box, line: Line): void {
  const plan = displayPlan(boxEnv(b));
  if ("error" in plan) return line(false, `display: ${plan.error}`, [`fix ${join(b.repo, ".env")} (.env.example)`]);
  const d = daemonInfo(b);
  const xvfb = sh("sh", ["-c", "command -v Xvfb"]);
  if (plan.how === "fixed") {
    const n = plan.display.slice(1).split(".")[0];
    const up = existsSync(`/tmp/.X11-unix/X${n}`);
    line(up, `display: ${plan.display} from TERRA_RENDER_DISPLAY${up ? "" : ": no X server there"}`, ["start one there, or empty TERRA_RENDER_DISPLAY for the daemon's own Xvfb"]);
  } else if (d?.alive && d.display) {
    const n = /Xvfb :(\d+)/.exec(d.display)?.[1];
    const up = !n || existsSync(`/tmp/.X11-unix/X${n}`);
    line(up && (xvfb.ok || !d.display.startsWith("Xvfb")), `display: jobs get ${d.display}${up ? "" : ", but its socket is gone"}${xvfb.ok ? "" : "; Xvfb is not installed"}`, ["sudo apt-get install -y xvfb; the daemon starts it again by itself"]);
  } else line(plan.how === "inherit" || xvfb.ok, `display: ${plan.how === "inherit" ? plan.said : xvfb.ok ? `none set; the daemon starts ${xvfb.out} when a job first needs it` : "none set and Xvfb is not installed: every sandbox job panics"}`, ["sudo apt-get install -y xvfb"]);
  const libs = ["libX11.so.6", "libXcursor.so.1", "libXi.so.6", "libXrandr.so.2", "libxkbcommon-x11.so.0"];
  const have = sh("ldconfig", ["-p"]).out;
  const missing = libs.filter((l) => !have.includes(`${l} `));
  line(!missing.length, missing.length ? `X client libraries missing: ${missing.join(", ")} (winit cannot open the display)` : "X client libraries: X11, Xcursor, Xi, Xrandr, xkbcommon-x11", [
    "sudo apt-get install -y libx11-6 libxcursor1 libxi6 libxrandr2 libxkbcommon-x11-0",
  ]);
}

/// Which renderer Blender gets: over an Xvfb its OpenGL is Mesa's llvmpipe, software, about 20 s
/// an Eevee frame; its Vulkan backend reaches the NVIDIA card with no display. Jobs get
/// TERRA_BLENDER_GPU_BACKEND=vulkan (jobs.ts jobEnv), which tools/blender/run.sh passes on.
function blender(b: Box, line: Line): void {
  const found = sh(join(b.repo, "tools/blender/run.sh"), ["--find"]);
  if (!found.ok) return line("warn", "Blender: none found (BLENDER, or blender on the PATH); creature and model jobs cannot run");
  const probe = "import gpu\ntry:\n    gpu.init()\n    print('GPU', gpu.platform.backend_type_get(), '|', gpu.platform.renderer_get())\nexcept Exception as e:\n    print('GPU none |', e)";
  const d = daemonInfo(b);
  const dpy = /Xvfb (:\d+)/.exec(d?.display ?? "")?.[1];
  const env = { ...process.env, ...(dpy ? { DISPLAY: dpy } : {}) };
  const renderer = (backend: string[]) => {
    const r = spawnSync(found.out, ["--background", ...backend, "--factory-startup", "--python-expr", probe], { encoding: "utf8", env, timeout: 60_000 });
    return /^GPU (.*)$/m.exec(r.stdout ?? "")?.[1]?.trim() ?? "no answer";
  };
  const vulkan = renderer(["--gpu-backend", "vulkan"]);
  line(/NVIDIA/i.test(vulkan), `Blender (jobs, --gpu-backend vulkan): ${vulkan}`, ["the NVIDIA Vulkan ICD (see Vulkan above); else Blender renders on the CPU, ~20 s an Eevee frame"]);
  const gl = renderer([]);
  line(/llvmpipe|softpipe|software/i.test(gl) ? "warn" : true, `Blender over ${dpy ?? "the daemon's display"}, OpenGL: ${gl}${/llvmpipe/i.test(gl) ? " (software; jobs use Vulkan instead)" : ""}`);
}

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

  display(b, line);
  blender(b, line);

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
  const magick = sh("magick", ["-version"]).ok;
  line(magick || "warn", magick ? "ImageMagick 7 (magick)" : "magick: not on the PATH; sheets and look checks cannot compose", [
    "ImageMagick's AppImage from github.com/ImageMagick/ImageMagick/releases: --appimage-extract into ~/opt, AppRun linked as ~/.local/bin/magick",
  ]);

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
