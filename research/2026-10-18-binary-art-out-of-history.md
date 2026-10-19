# Binary art out of git history

2026-10-18. The question: the repository's binaries make it "painful to work with", so what is the most seamless way to move them out of git history, what does each worktree then cost, and what would Git LFS on GitHub cost the repository's owner (dcramer)? Measured on this machine (btrfs `/home`) against `main` at 1484d669, with a rewrite rehearsed on a bare copy in `~/.cache/terra-review/lfs-rehearsal`.

## Answer

- **Git LFS on GitHub, with the whole history migrated** (`git lfs migrate import --everything`), is the seamless option: git's filters fetch and write the files, so no tool or agent changes how it works. Git history goes from **1.93 GiB to 10.8 MiB**; the binaries, 2.3 GB in every version, move to LFS.
- **Worktrees cost almost nothing with it.** `git lfs checkout` writes files as btrfs reflinks of the shared store in `.git/lfs/objects`: a worktree's binaries took 26 MiB of their own instead of 1.07 GiB. A plain `git worktree add` copies them instead, so new worktrees check out with smudging skipped and fill in with `git lfs checkout` (the seed step, D183, is where that goes).
- **The cost to the owner is small**: nothing for about the first month, then roughly $1–5 a month by six months and $2–10 by a year at the present pace (below). He has to set a payment method and a budget before storage passes 10 GiB, or pushes of new binaries stop.
- **The rewrite changes every commit hash**, so it needs a force-push of `main`, which AGENTS.md forbids without the developers, and every checkout must be re-cloned or reset. Commit hashes quoted in docs and decisions stop resolving on GitHub unless mapped.

## What is in history

| Kind | In history (on disk) | Versions |
|---|---|---|
| `.glb` | 1,288 MB | 614 |
| `.png` | 406 MB | 349 |
| `.jpg` | 374 MB | 230 |
| everything else | about 25 MB | 9,700 |

At HEAD the binaries are 1.1 GB: `art/` 704 MB (source art the game never loads), `assets/` 401 MB. In history `art/` is 1,031 MB and `assets/` 948 MB.

New binary bytes reaching `main` a day: 208, 703, 255, 556, 471 MB on 11–15 October while the kits and props were generated, then 91, 105 and 21 MB on 16–18 October. Over the nine days, 2.45 GB in all.

## Options

| Option | History | Worktree | Seamless | Cost |
|---|---|---|---|---|
| **LFS on GitHub, history migrated** | 11 MiB | ~26 MiB with reflinks | Yes: git filters; needs `git-lfs` installed (mise) | GitHub LFS, below |
| LFS on GitHub, new commits only | 2.2 GB, stops growing | Same | Yes | Same, minus the 2.3 GB already in history |
| LFS on our own server over Cloudflare R2 (a Worker answering LFS's batch API) | 11 MiB | Same | Yes once set up; we run the server and its write tokens | R2: 10 GB free, $0.015/GB-month after, no egress fees |
| Object store and a fetch tool (manifest in git, like DVC) | 11 MiB | Reflinks from our own store | No: a step of our own after every checkout and before every commit | R2 as above |
| Partial clone (`--filter=blob:none`) and sparse checkout of `art/` | Unchanged on GitHub, fetched lazily | 400 MB without `art/` | Mostly; tools reading `art/` must widen the checkout | None |
| No LFS, reflinks from a blob store in the seed step | Unchanged | 15 MiB (measured) | Yes | None; history keeps growing |

## What GitHub LFS costs

From GitHub's billing docs: personal Free and Pro accounts include **10 GiB storage and 10 GiB bandwidth a month**, account-wide (the owner's other repositories share it), then **$0.07 per GiB-month storage** and **$0.0875 per GiB downloaded**. Uploads are free. Storage counts every version ever pushed and only falls when the repository is deleted. Downloads by anyone, forks and strangers cloning this public repository included, bill the owner. With no payment method, or a $0 budget, going over storage stops pushes of new LFS files, and going over bandwidth turns LFS off until the next month.

Estimates, at 70 MB a day (the last three days) to 300 MB a day (the nine-day mean):

| | Storage | Over 10 GiB | Storage $/month |
|---|---|---|---|
| Migration day | 2.3 GiB | 0 | $0 |
| 1 month | 4–11 GiB | 0–1 | $0–0.10 |
| 6 months | 15–56 GiB | 5–46 | $0.35–3.20 |
| 12 months | 28–110 GiB | 18–100 | $1.30–7.00 |

Bandwidth: this machine's agents fetch from the local store and download nothing; its pushes are uploads. Each other machine (the owner's, the CI box) downloads 1.1 GiB once and then the new binaries it pulls, 2–9 GiB a month each. Two other machines: 4–18 GiB a month, $0–0.70 over the free 10. A stranger's clone costs about $0.10.

So roughly **$1–5 a month at six months, $2–10 a month at a year**, rising with art production. A budget of $10–20 a month caps it. Moving source art that is regenerated often (`art/`) to R2 instead would cut the storage share that grows fastest.

## Rehearsal

On a bare single-branch copy of `main` (1,165 commits), `git lfs migrate import --everything` with the binary patterns took 16 s; after `gc --aggressive` the pack is 10.8 MiB and `lfs/` 2.3 GB. A clone with smudging skipped, then `git lfs fetch` and `git lfs checkout`, left `git status` clean and its 1.05 GiB of binaries shared with `.git/lfs` (btrfs `filesystem du`). A `git worktree add` on the same clone wrote 1.07 GiB of its own.

## Sources

- GitHub, [Git LFS billing](https://docs.github.com/en/billing/concepts/product-billing/git-lfs) and [about billing for Git LFS](https://docs.github.com/en/billing/using-the-new-billing-platform/about-billing-for-git-large-file-storage).
- git-lfs worktree support: storage lives in the common git directory ([git-lfs issue 766](https://github.com/github/git-lfs/issues/766)).
