# Development

How to set up a machine and work in the repository. The reasons behind these choices are in [tech.md](tech.md) and [decisions.md](decisions.md).

## Set up

Install [rustup](https://rustup.rs), clone the repository, then from its root:

```bash
cargo xtask doctor
```

It reads the pinned compiler version from `rust-toolchain.toml`, installs nothing itself, and lists what is missing with the command that fixes each item. The first `cargo` command in the repository downloads the pinned compiler.

| Platform | What you need beyond rustup |
|---|---|
| Linux, including WSL | `pkg-config`, `libasound2-dev`, `libudev-dev`, `libwayland-dev`, `libxkbcommon-dev` |
| WSL, to use the Windows GPU | `mingw-w64`, and the `x86_64-pc-windows-gnu` Rust target |
| macOS | Xcode command line tools |

macOS has been checked only by CI, which builds and tests there. Nobody has run a window on a Mac yet.

## Everyday commands

| Command | What it does |
|---|---|
| `cargo xtask check` | Formatting, lints, tests and the dependency rule. Must pass before a commit. |
| `cargo test -p terra_sim` | One crate's tests |
| `cargo fmt --all` | Format everything |
| `cargo xtask doctor` | Check this machine |

A clean build of a Bevy application takes about two and a half minutes; a rebuild after a small change takes a few seconds.

## Seeing and measuring

WSL has no hardware Vulkan, which splits the work in two:

- **To see output**, run the Linux build. It renders on the CPU, slowly but correctly, and can save screenshots. This works anywhere, including for agents.
- **To measure performance**, cross-compile for Windows and run that build on the real GPU:

```bash
cargo build --target x86_64-pc-windows-gnu
```

Two things to know when launching a Windows build from WSL. Start it from a folder on a Windows drive, because a Windows program started from a WSL folder gets an unusable working directory. And keep debug builds on the workspace's default profile, because full debug information produces a file Windows refuses to load.

Every application we build gets scripted modes that fly a fixed camera path, print frame times and save screenshots, so results can be compared between machines and over time.

## Conventions

- The rules for working in the workspace are in [AGENTS.md](../AGENTS.md). They apply to people as well as agents.
- How crates are split, and how to add one, is in [crates/README.md](../crates/README.md).
- Dependencies are pinned through `Cargo.lock`, which is committed.
- Bevy is upgraded deliberately, once the crates we depend on support the new release (D31).

## Continuous integration

Every push and pull request runs `cargo xtask check` on Linux and macOS, and a compile check for the Windows target. Nothing in CI uses a GPU.
