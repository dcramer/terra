#!/bin/sh
# Makes a render box's folder ready for `cargo xtask remote-render --update` (docs/render-worker.md),
# which sends this over ssh: `sh -s -- <dir> [node]`. Idempotent, and needs no sudo: the clone
# pushes land in, and with `node` the official Node 22 LTS tarball in <dir>/node, its checksum
# checked against nodejs.org's SHASUMS256.txt.
set -eu
dir="$1"
case "$dir" in /*) ;; *) dir="$HOME/$dir" ;; esac
mkdir -p "$dir/jobs" "$dir/home"
[ -d "$dir/repo/.git" ] || git init -q "$dir/repo"
[ "${2:-}" = node ] || exit 0
if [ -x "$dir/node/bin/node" ] && "$dir/node/bin/node" -e 'process.exit(+process.versions.node.split(".")[0] >= 22 ? 0 : 1)'; then exit 0; fi
case "$(uname -m)" in x86_64) arch=x64 ;; aarch64) arch=arm64 ;; *) echo "✗ bootstrap: no Node build for $(uname -m)"; exit 1 ;; esac
base=https://nodejs.org/dist/latest-v22.x
tmp="$dir/node.tmp"
rm -rf "$tmp" && mkdir -p "$tmp"
curl -fsSL "$base/SHASUMS256.txt" -o "$tmp/SHASUMS256.txt"
file=$(awk "/node-v[0-9.]*-linux-$arch.tar.xz\$/ {print \$2}" "$tmp/SHASUMS256.txt")
[ -n "$file" ] || { echo "✗ bootstrap: no linux-$arch tarball listed at $base"; exit 1; }
curl -fsSL "$base/$file" -o "$tmp/$file"
(cd "$tmp" && grep " $file\$" SHASUMS256.txt | sha256sum -c --status) || { echo "✗ bootstrap: $file does not match its checksum"; exit 1; }
tar -xJf "$tmp/$file" -C "$tmp"
rm -rf "$dir/node" && mv "$tmp/${file%.tar.xz}" "$dir/node" && rm -rf "$tmp"
echo "bootstrap: installed $("$dir/node/bin/node" --version) in $dir/node"
