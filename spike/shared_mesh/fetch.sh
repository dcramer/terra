#!/usr/bin/env bash
# Fetches MakeHuman's CC0 data (base mesh, targets, rigs, weights, poses) from MPFB2's
# release into spike/shared_mesh/mh/, which is ignored.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
[[ -f "$here/mh/3dobjs/base.obj" ]] && exit 0
tmp=$(mktemp -d)
curl -sL https://github.com/makehumancommunity/mpfb2/archive/refs/tags/v2.0.18.tar.gz | tar -xz -C "$tmp" mpfb2-2.0.18/src/mpfb/data mpfb2-2.0.18/LICENSE.md
mkdir -p "$here/mh"
cp -r "$tmp"/mpfb2-2.0.18/src/mpfb/data/. "$here/mh/"
cp "$tmp"/mpfb2-2.0.18/LICENSE.md "$here/mh/"
rm -rf "$tmp"
echo "FETCHED $(du -sh "$here/mh" | cut -f1)"
