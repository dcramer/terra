# Rebuilds

How long the sandbox takes to build again after one file changes, and which crates were compiled for it (D407): the measure each step of cutting the crates is checked against.

```bash
cargo xtask rebuilds
```

For each probe, one file in each part of the code that is or will be its own crate (`PROBES` in `rebuilds.ts`), it touches the file and rebuilds `terra_sandbox` with `--timings`, in dev and then in release (`--dev` or `--release` for one; probe names to run only those). A line a probe: the whole rebuild's seconds, last run's beside it, the machine's load average against its cores, and each workspace crate compiled with its seconds from cargo's own report. The numbers go to `target/rebuilds/last.json`, cargo's output to `target/rebuilds/cargo.log`.

A touch changes no code, so it measures what cargo and rustc redo when a file's time changes: every crate above it is compiled again, rustc's incremental cache reusing what it can in dev and nothing in release. Times on a loaded machine vary; compare runs at a similar load. A release pass takes many minutes: run it in the background.
