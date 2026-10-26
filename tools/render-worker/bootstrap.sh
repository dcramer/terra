#!/bin/sh
# Makes a render box's folder ready for `cargo xtask remote-render --update` (docs/render-worker.md),
# which sends this over ssh: `sh -s -- <dir> [node]`. Idempotent, and needs no sudo: the clone
# pushes land in, and with `node` the official Node 22 LTS tarball in <dir>/node, its checksum
# checked against nodejs.org's SHASUMS256.txt, and Git LFS's official release in <dir>/git-lfs,
# checked against its sha256sums, set as the clone's own filter (D525): the client rsyncs a job's
# LFS objects into the clone's store and its checkouts smudge from there, never from GitHub.
set -eu
dir="$1"
case "$dir" in /*) ;; *) dir="$HOME/$dir" ;; esac
mkdir -p "$dir/jobs" "$dir/homes"
[ -d "$dir/repo/.git" ] || git init -q "$dir/repo"
mkdir -p "$dir/repo/.git/lfs/objects"
[ "${2:-}" = node ] || exit 0
case "$(uname -m)" in x86_64) arch=x64 lfsarch=amd64 ;; aarch64) arch=arm64 lfsarch=arm64 ;; *) echo "✗ bootstrap: no Node build for $(uname -m)"; exit 1 ;; esac

lfs_version=3.8.1
lfs="$dir/git-lfs/git-lfs"
if ! "$lfs" version 2>/dev/null | grep -q "^git-lfs/$lfs_version "; then
  base="https://github.com/git-lfs/git-lfs/releases/download/v$lfs_version"
  file="git-lfs-linux-$lfsarch-v$lfs_version.tar.gz"
  tmp="$dir/git-lfs.tmp"
  rm -rf "$tmp" && mkdir -p "$tmp"
  curl -fsSL "$base/sha256sums.asc" -o "$tmp/sha256sums.asc"
  curl -fsSL "$base/$file" -o "$tmp/$file"
  (cd "$tmp" && grep -F " *$file" sha256sums.asc | sha256sum -c --status) || { echo "✗ bootstrap: $file does not match its checksum"; exit 1; }
  tar -xzf "$tmp/$file" -C "$tmp"
  rm -rf "$dir/git-lfs" && mv "$tmp/git-lfs-$lfs_version" "$dir/git-lfs" && rm -rf "$tmp"
  echo "bootstrap: installed $("$lfs" version | cut -d' ' -f1) in $dir/git-lfs"
fi
cd "$dir/repo"
git config filter.lfs.process "$lfs filter-process"
git config filter.lfs.smudge "$lfs smudge -- %f"
git config filter.lfs.clean "$lfs clean -- %f"
git config filter.lfs.required true

if [ -x "$dir/node/bin/node" ] && "$dir/node/bin/node" -e 'process.exit(+process.versions.node.split(".")[0] >= 22 ? 0 : 1)'; then exit 0; fi
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
