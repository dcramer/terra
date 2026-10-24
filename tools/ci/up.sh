#!/bin/sh
# The CI agent's box in one command (docs/ci-agent.md), run over SSH on the machine:
#   curl -fsSL https://raw.githubusercontent.com/dcramer/terra/main/tools/ci/up.sh | sh
# It needs Docker, and GH_TOKEN in the environment the first time (a classic token with the
# public_repo scope), kept in ~/terra-ci/.env after. Running it again updates and restarts it.
set -eu
repo="${TERRA_REPO:-dcramer/terra}"
dir="${TERRA_CI_DIR:-$HOME/terra-ci}"
command -v docker >/dev/null || { echo "✗ terra-ci: install Docker first: https://docs.docker.com/engine/install/"; exit 1; }
docker info >/dev/null 2>&1 || { echo "✗ terra-ci: docker is not reachable; start it, or let this user use it: sudo usermod -aG docker \$USER, then log in again"; exit 1; }
docker compose version >/dev/null 2>&1 || { echo "✗ terra-ci: needs Docker Compose v2 (the docker compose plugin)"; exit 1; }
mkdir -p "$dir"
cd "$dir"
if [ -n "${GH_TOKEN:-}" ]; then
  (umask 077 && printf 'GH_TOKEN=%s\nTERRA_REPO=%s\n' "$GH_TOKEN" "$repo" > .env)
fi
[ -f .env ] || { echo "✗ terra-ci: set GH_TOKEN (a classic token with the public_repo scope) and run it again"; exit 1; }
for f in Dockerfile compose.yaml entrypoint.sh; do
  curl -fsSL "https://raw.githubusercontent.com/$repo/main/tools/ci/$f" -o "$f"
done
chmod +x entrypoint.sh
docker compose up -d --build --quiet-pull > build.log 2>&1 || { tail -20 build.log; echo "✗ terra-ci: the build failed; $dir/build.log has all of it"; exit 1; }
echo "✓ terra-ci is running from $dir; its first run builds everything, then it follows main"
echo "  watch it: docker compose -f $dir/compose.yaml logs -f"
