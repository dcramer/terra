// Whether a push is one the pre-push hook's checks guard (D37): a push to origin, any ref of it,
// since that is how work lands. A push elsewhere, such as remote-render sending a job's commit to
// the render worker (refs/jobs/<id>, refs/worker/main, D445), is not landing, so the hook lets it
// through unchecked rather than run clippy here and fail it as "failed to push some refs".
//
//   node tools/git/guarded.ts <remote> <url>   the hook's own two arguments; exits 0 when the
//                                              push goes to origin (by name or by its URL)
//
// Node 22.18+, no dependencies; `node --test tools/git/guarded.test.ts`.

import { spawnSync } from "node:child_process";

/// A remote URL in one spelling: no trailing slash or `.git`, so two spellings of one compare equal.
const norm = (url: string) => url.trim().replace(/\/+$/, "").replace(/\.git$/, "");

/// True when a push to `remote` at `url` goes to origin: the remote named origin, or a URL that is
/// origin's (`git push <origin's url>` passes the URL as both arguments).
export function guarded(remote: string, url: string, originUrls: string[]): boolean {
  if (remote === "origin") return true;
  const urls = new Set(originUrls.filter(Boolean).map(norm));
  return [remote, url].some((u) => u && urls.has(norm(u)));
}

function main(argv: string[]): number {
  const [remote = "", url = ""] = argv;
  const origin = spawnSync("git", ["remote", "get-url", "--all", "--push", "origin"], { encoding: "utf8" });
  const fetch = spawnSync("git", ["remote", "get-url", "--all", "origin"], { encoding: "utf8" });
  const urls = `${origin.stdout ?? ""}\n${fetch.stdout ?? ""}`.split("\n");
  return guarded(remote, url, urls) ? 0 : 1;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
