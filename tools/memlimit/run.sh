#!/usr/bin/env bash
# Cargo's runner on Linux (.cargo/config.toml): every test, bench and `cargo run` binary starts
# under a cap on its heap, so one that runs away fails alone instead of filling WSL's memory.
#
#   TERRA_MEM_LIMIT=48G cargo test -p terra_world    # raise it for one run; 0 lifts it
#
# The cap is RLIMIT_DATA (default 32G): past it an allocation fails and Rust aborts with
# "memory allocation of N bytes failed". Unchecked, a terra_world test grew to 89 GB, WSL's
# whole share, and the kernel's out-of-memory kill took every shell and agent down with it.
set -euo pipefail
limit=${TERRA_MEM_LIMIT:-32G}
if [ "$limit" != 0 ]; then
    kb=$(numfmt --from=iec "$limit")
    ulimit -d $((kb / 1024))
fi
exec "$@"
