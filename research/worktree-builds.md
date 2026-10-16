# Building in many worktrees: what makes a new one quick

2026-10-12. Question: agents each work in their own git worktree, and every new worktree built all of Bevy from nothing. What is the best way to share that work between checkouts on one machine?

## Answer

Put the main checkout's compiled dependencies into the new worktree's `target/` before it builds, hard-linking the libraries and copying the rest, and leave out our own crates (D183). A fresh worktree then compiles only Terra's seven crates. A compiler cache (sccache) saved nothing, and a build folder shared between checkouts is unsafe.

| Measured on the WSL machine (32 threads), `cargo build -p terra_sandbox`, Rust 1.97.1 | Time |
|---|---|
| A new worktree building from nothing | 5 min 15 s to 6 min 42 s |
| The same with sccache, cache filled by another worktree (65% hits) | 5 min 50 s |
| Seeding from the main checkout: the copy, 10.5 GB of Linux and Windows dependencies | 4 s |
| The same with the libraries hard-linked (2026-10-13): 12.2 GB linked, 0.17 GB copied | 3 s |
| Then the first build, which compiles only our crates | 60 s |
| Then the first Windows build (`--target x86_64-pc-windows-gnu`) | 63 s |
| Then `cargo xtask check` | 120 s |

## What was tried

**sccache**, the usual compiler cache. Dependencies come from `~/.cargo/registry`, so their sources sit at the same path in every worktree, yet a third of them missed. Each crate with a build script gets its `OUT_DIR` under the worktree's own `target/`. sccache hashes that path, and the build script's output carries it into the compiled crate, so every crate above it misses too. Those are the large ones: serde, wgpu, winit, wayland and all of Bevy. sccache's `basedirs` option, meant to fix this, applies only to C and C++. It cannot cache our own crates either, since they are built incrementally. The cache added overhead and saved nothing, so it is not used.

**A shared build folder.** Cargo 1.91 split `build.build-dir`, for intermediate files, from the target folder that holds the programs. Pointing every checkout at one build folder made a second worktree's build take half a second. It is wrong, though. Cargo hashes our own crates by package rather than by path, so two worktrees share one unit of `terra_core`, and Cargo judges it fresh by file times. An edit built in one worktree was handed to the other with no rebuild: tested, and it happened. Checksum freshness (`-Zchecksum-freshness`) would fix that but is unstable. Builds in different worktrees would also wait on one another's lock (`-Zfine-grain-locking` is unstable too).

**Seeding**, which is what we use. Dependencies are fingerprinted by version, features and profile, not by file times or by where the target folder is, and Cargo rewrites a build script's recorded `OUT_DIR` when the target folder has moved. So a copy of main's dependency units is fresh in the worktree. Our own crates' units are left out, because their freshness comes from file times. They are rebuilt, which is correct whatever the two checkouts hold. The copy keeps each file's modification time. `std::fs::copy` uses `copy_file_range` on Linux and clones on APFS, Btrfs and XFS, so on those the copy takes no space. Only `deps/`, `build/` and `.fingerprint/` are copied. Fingerprints go last, so a copy cut short leaves units Cargo simply rebuilds.

The seed follows Cargo's locking. Cargo 1.97 holds `.cargo-lock` shared and `.cargo-build-lock` and `.cargo-artifact-lock` exclusive in a profile folder while it builds, and lets them go before `cargo run` starts the program. Both were checked. The seed holds main's build lock shared, so a build there finishes before the copy, and holds the worktree's locks exclusive while it copies. A `cargo` command started meanwhile prints "Blocking waiting for file lock" and waits for it.

**Hard links** (added 2026-10-13). On ext4, which cannot clone a file, each seed was 10.5 GB of real disk, and ten subagents working at once held over 130 GB. Hard-linking everything would be wrong: Cargo and rustc rewrite fingerprint and dep-info files in place, so a worktree's rebuild would change the main checkout's state. But rustc writes an `.rlib` or `.rmeta` to a temporary file and renames it into place, and LLD does the same for a proc-macro `.so`. Tested with Rust 1.97.1 by deleting a seeded dependency's fingerprint so it rebuilt under the same name: the `.rlib`, `.rmeta` and `.so` came out as new files and the main checkout's were unchanged, while the `.d` beside them was rewritten through the link. So the seed links those three kinds, about 98% of the size, and copies the rest. A link that fails, as across file systems, falls back to a copy. Even if a later rustc wrote one in place, it would be a rebuild of the same crate, version, features and compiler.

## The shared seed (added 2026-10-16)

Seeding from the main checkout only helped while that checkout had built recently. Often its build was missing, or behind main's `Cargo.lock`, and new worktrees built from nothing (10 to 20 minutes with many agents building at once). The seed now comes from a build of origin/main kept for this alone in `.git/terra-seed/`: its files are checked out through an index of its own, so it is not a worktree and no worktree tool lists or clears it. It is built again under a lock whenever origin/main's manifests change (D183). It is built by the commands agents run, since Cargo builds a dependency again for each set of features the packages chosen ask of it: clippy and the tests over the workspace, the kit and artifact examples, and the sandbox, with the Windows sandbox in debug and release under WSL.

| Measured on the WSL machine (32 threads), Rust 1.97.1 | Time |
|---|---|
| The seed built from nothing, all seven builds, other work on the machine | 17 min |
| A fresh worktree's first `cargo xtask check`, seeded from the main checkout's build (current that day) | 4 min 14 s |

## Claude Code's side

- A `WorktreeCreate` hook replaces Claude Code's own `git worktree add` rather than adding to it, so it is not used.
- `SessionStart` runs the seed. For a session in a worktree, `CLAUDE_PROJECT_DIR` is the main checkout, so the hook reads the worktree from the `cwd` it is given. The hook returns in about a second: it builds `xtask`, which has no dependencies, and waits only until the seed holds the locks.
- `SubagentStart` runs the same hook for subagents that work in their own worktree. What `cwd` such a hook receives is not documented, and the hook does nothing when `cwd` is not a fresh worktree.
- A worktree entered partway through a session gets no hook, so `cargo xtask seed` is there to run by hand.
- `.worktreeinclude` copies ignored files into new worktrees. Nothing needs it: the token stats read the main checkout's `.env` already.

## Also checked

- **Linker.** Since Rust 1.90, Linux builds link with LLD by default, and ours already do. mold would gain little more.
- **Cranelift**, the faster debug code generator, needs a nightly compiler. We pin a stable one.
- **Bevy's `dynamic_linking`** shortens linking after a small change. It has nothing to do with worktrees, and it would ship a DLL beside the Windows build that `cargo xtask win` does not copy. Not looked at further.
