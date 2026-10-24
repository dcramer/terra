// Where render jobs and their files are kept (docs/render-worker.md, D445). `GitHub`: a draft
// release a job, through `gh`; only people with push access can make or see one. `Folder`: a
// folder a job, for a run on one machine with no GitHub (`--local`, the tests).

import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { BEAT, decode, encode, type Job, type Rec, TAG } from "./jobs.ts";

export interface Store {
  /// Every job, oldest first.
  list(): Rec[];
  create(job: Job): Rec;
  save(rec: Rec, job: Job): void;
  upload(rec: Rec, files: string[]): void;
  /// Downloads every file of the job into `dir`.
  download(rec: Rec, dir: string): void;
  remove(rec: Rec): void;
  beat(text: string): void;
  /// The worker's last heartbeat: when and what it said.
  seen(): { when: string; text: string } | undefined;
  where(rec: Rec): string;
}

function gh(args: string[], input?: string): string {
  const r = spawnSync("gh", args, { encoding: "utf8", input, stdio: [input === undefined ? "ignore" : "pipe", "pipe", "pipe"], maxBuffer: 64 * 1024 ** 2 });
  if (r.status !== 0) throw new Error(`gh ${args.slice(0, 3).join(" ")}: ${(r.stderr || r.error?.message || "").trim().split("\n").at(-1)}`);
  return r.stdout;
}

type Release = { id: number; tag_name: string; draft: boolean; body: string | null; html_url: string; assets: { name: string }[] };

export class GitHub implements Store {
  private repo: string;
  constructor(repo: string) {
    this.repo = repo;
  }

  /// The repository `gh` sees from `cwd`, or TERRA_REPO.
  static from(cwd: string): GitHub {
    if (process.env.TERRA_REPO) return new GitHub(process.env.TERRA_REPO);
    const r = spawnSync("gh", ["repo", "view", "--json", "nameWithOwner", "-q", ".nameWithOwner"], { cwd, encoding: "utf8" });
    if (r.status !== 0) throw new Error("gh cannot see the repository; run `gh auth login`, or set TERRA_REPO");
    return new GitHub(r.stdout.trim());
  }

  private releases(): Release[] {
    // Newest first; a job is deleted after a week, so the newest hundred hold every one.
    return (JSON.parse(gh(["api", `repos/${this.repo}/releases?per_page=100`])) as Release[]).filter((r) => r.draft && r.tag_name.startsWith(TAG));
  }

  list(): Rec[] {
    const recs: Rec[] = [];
    for (const r of this.releases()) {
      const job = r.tag_name === BEAT ? undefined : decode(r.body ?? "");
      if (job) recs.push({ ref: String(r.id), job, files: r.assets.map((a) => a.name) });
    }
    return recs.sort((a, b) => a.job.id.localeCompare(b.job.id));
  }

  create(job: Job): Rec {
    const body = JSON.stringify({ tag_name: TAG + job.id, name: `render job ${job.id}`, body: encode(job), draft: true });
    const made = JSON.parse(gh(["api", "-X", "POST", `repos/${this.repo}/releases`, "--input", "-"], body)) as Release;
    return { ref: String(made.id), job, files: [] };
  }

  save(rec: Rec, job: Job): void {
    gh(["api", "-X", "PATCH", `repos/${this.repo}/releases/${rec.ref}`, "--input", "-"], JSON.stringify({ body: encode(job) }));
    rec.job = job;
  }

  upload(rec: Rec, files: string[]): void {
    gh(["release", "upload", TAG + rec.job.id, ...files, "-R", this.repo, "--clobber"]);
  }

  download(rec: Rec, dir: string): void {
    mkdirSync(dir, { recursive: true });
    gh(["release", "download", TAG + rec.job.id, "-R", this.repo, "-D", dir, "--clobber"]);
  }

  remove(rec: Rec): void {
    gh(["api", "-X", "DELETE", `repos/${this.repo}/releases/${rec.ref}`]);
  }

  beat(text: string): void {
    const body = `${new Date().toISOString()} ${text}`;
    const found = this.releases().find((r) => r.tag_name === BEAT);
    if (found) gh(["api", "-X", "PATCH", `repos/${this.repo}/releases/${found.id}`, "--input", "-"], JSON.stringify({ body }));
    else gh(["api", "-X", "POST", `repos/${this.repo}/releases`, "--input", "-"], JSON.stringify({ tag_name: BEAT, name: "render worker", body, draft: true }));
  }

  seen(): { when: string; text: string } | undefined {
    const body = this.releases().find((r) => r.tag_name === BEAT)?.body ?? "";
    const m = /^(\S+) (.*)$/s.exec(body);
    return m ? { when: m[1], text: m[2] } : undefined;
  }

  where(rec: Rec): string {
    return `https://github.com/${this.repo}/releases (draft ${TAG}${rec.job.id})`;
  }
}

export class Folder implements Store {
  private dir: string;
  constructor(dir: string) {
    this.dir = dir;
    mkdirSync(dir, { recursive: true });
  }

  list(): Rec[] {
    const recs: Rec[] = [];
    for (const name of readdirSync(this.dir).sort()) {
      const file = join(this.dir, name, "job.json");
      if (!name.startsWith(TAG) || name === BEAT || !existsSync(file)) continue;
      const files = existsSync(join(this.dir, name, "files")) ? readdirSync(join(this.dir, name, "files")) : [];
      recs.push({ ref: join(this.dir, name), job: JSON.parse(readFileSync(file, "utf8")), files });
    }
    return recs;
  }

  create(job: Job): Rec {
    const ref = join(this.dir, TAG + job.id);
    mkdirSync(ref, { recursive: true });
    writeFileSync(join(ref, "job.json"), JSON.stringify(job, null, 2));
    return { ref, job, files: [] };
  }

  save(rec: Rec, job: Job): void {
    writeFileSync(join(rec.ref, "job.json"), JSON.stringify(job, null, 2));
    rec.job = job;
  }

  upload(rec: Rec, files: string[]): void {
    mkdirSync(join(rec.ref, "files"), { recursive: true });
    for (const f of files) copyFileSync(f, join(rec.ref, "files", basename(f)));
  }

  download(rec: Rec, dir: string): void {
    mkdirSync(dir, { recursive: true });
    const from = join(rec.ref, "files");
    for (const f of existsSync(from) ? readdirSync(from) : []) copyFileSync(join(from, f), join(dir, f));
  }

  remove(rec: Rec): void {
    rmSync(rec.ref, { recursive: true, force: true });
  }

  beat(text: string): void {
    writeFileSync(join(this.dir, BEAT), `${new Date().toISOString()} ${text}`);
  }

  seen(): { when: string; text: string } | undefined {
    const file = join(this.dir, BEAT);
    const m = existsSync(file) ? /^(\S+) (.*)$/s.exec(readFileSync(file, "utf8")) : null;
    return m ? { when: m[1], text: m[2] } : undefined;
  }

  where(rec: Rec): string {
    return rec.ref;
  }
}

/// The store a run uses: a folder when TERRA_RENDER_STORE names one (`dir:<path>`), else GitHub.
export function open(cwd: string, local?: string): Store {
  const spec = local ? `dir:${local}` : (process.env.TERRA_RENDER_STORE ?? "");
  return spec.startsWith("dir:") ? new Folder(spec.slice(4)) : GitHub.from(cwd);
}
