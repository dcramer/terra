#!/bin/sh
# The CI agent's box in one command (docs/ci-agent.md), run over SSH on the machine:
#   curl -fsSL https://raw.githubusercontent.com/dcramer/terra/main/tools/ci/up.sh | sh
# It needs Docker, and GH_TOKEN in the environment the first time (a classic token with the
# public_repo scope), kept in ~/terra-ci/.env after. Running it again updates and restarts it.
# TERRA_RENDER=1 adds the render worker (docs/render-worker.md), which needs an NVIDIA card and
# the NVIDIA Container Toolkit; TERRA_RENDER=0 removes it. Either is kept in .env.
set -eu
repo="${TERRA_REPO:-dcramer/terra}"
dir="${TERRA_CI_DIR:-$HOME/terra-ci}"
command -v docker >/dev/null || { echo "✗ terra-ci: install Docker first: https://docs.docker.com/engine/install/"; exit 1; }
docker info >/dev/null 2>&1 || { echo "✗ terra-ci: docker is not reachable; start it, or let this user use it: sudo usermod -aG docker \$USER, then log in again"; exit 1; }
docker compose version >/dev/null 2>&1 || { echo "✗ terra-ci: needs Docker Compose v2 (the docker compose plugin)"; exit 1; }
mkdir -p "$dir"
cd "$dir"
# Sets KEY=VALUE in .env, keeping its other lines; an empty value removes the key.
set_env() {
  touch .env
  (umask 077 && { grep -v "^$1=" .env || true; } > .env.new && { [ -z "$2" ] || printf '%s=%s\n' "$1" "$2" >> .env.new; } && mv .env.new .env)
}
if [ -n "${GH_TOKEN:-}" ]; then
  set_env GH_TOKEN "$GH_TOKEN"
  set_env TERRA_REPO "$repo"
fi
case "${TERRA_RENDER:-}" in
  1) set_env COMPOSE_PROFILES render ;;
  0) set_env COMPOSE_PROFILES "" ;;
esac
grep -q '^GH_TOKEN=' .env 2>/dev/null || { echo "✗ terra-ci: set GH_TOKEN (a classic token with the public_repo scope) and run it again"; exit 1; }
for f in Dockerfile render.Dockerfile compose.yaml entrypoint.sh render-entrypoint.sh; do
  curl -fsSL "https://raw.githubusercontent.com/$repo/main/tools/ci/$f" -o "$f"
done
chmod +x entrypoint.sh render-entrypoint.sh
render=""
grep -q '^COMPOSE_PROFILES=.*render' .env && render=1
if [ -n "$render" ]; then
  command -v nvidia-ctk >/dev/null || echo "terra-render: no nvidia-ctk here; without the NVIDIA Container Toolkit the container sees no card (tools/render-worker/README.md)"
elif [ "${TERRA_RENDER:-}" = 0 ]; then
  docker compose --profile render rm -sf render >/dev/null 2>&1 || true
fi
docker compose up -d --build --quiet-pull > build.log 2>&1 || { tail -20 build.log; echo "✗ terra-ci: the build failed; $dir/build.log has all of it"; exit 1; }
echo "✓ terra-ci is running from $dir; its first run builds everything, then it follows main"
if [ -n "$render" ]; then
  if gpu="$(docker compose run --rm --no-deps --entrypoint nvidia-smi render --query-gpu=name,memory.total --format=csv,noheader 2>&1)"; then
    echo "✓ terra-render is running beside it, on $(echo "$gpu" | tail -1)"
  else
    echo "✗ terra-render: the container cannot reach the card: $(echo "$gpu" | tail -1) (tools/render-worker/README.md)"
    exit 1
  fi
fi
echo "  watch it: docker compose -f $dir/compose.yaml logs -f"
