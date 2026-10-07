#!/usr/bin/env bash
# Builds the spike for Linux and Windows and runs both. The Windows run opens a window on
# the GPU for about ten seconds.
set -u
cd "$(dirname "$0")"
T0=$(date +%s)
cargo build 2>&1 | grep -E '^error|Finished'
cargo build --target x86_64-pc-windows-gnu 2>&1 | grep -E '^error|Finished'
echo "both builds: $(( $(date +%s) - T0 )) s"
ls -la target/x86_64-pc-windows-gnu/debug/rust_stack_spike.exe | awk '{printf "windows exe: %.0f MB\n", $5/1048576}'
echo "--- linux (software renderer)"; timeout 200 ./target/debug/rust_stack_spike 2>&1 | grep -E '^SPIKE'
# A Windows program started from a WSL folder gets an unusable working directory, so run
# it from the Windows temp folder.
WIN="$(wslpath "$(cmd.exe /c 'echo %TEMP%' 2>/dev/null | tr -d '\r')")/terra_spike"
mkdir -p "$WIN"
cp target/x86_64-pc-windows-gnu/debug/rust_stack_spike.exe "$WIN/rust_stack_spike.exe"
echo "--- windows (GPU)"; (cd "$WIN" && timeout 120 ./rust_stack_spike.exe 2>&1 | tr -d '\r' | grep -E '^SPIKE')
