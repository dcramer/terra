#!/bin/sh
# The CI agent container's start (docs/ci-agent.md): sign git in with the token, clone the
# repository into the /work volume the first time, then `cargo xtask ci --watch`. Arguments, if
# any, go to the agent instead (`docker compose run --rm ci --dry-run`).
set -eu
: "${GH_TOKEN:?set GH_TOKEN in tools/ci/.env (see tools/ci/README.md)}"
repo="${TERRA_REPO:-dcramer/terra}"
gh auth setup-git
git config --global user.name "${GIT_AUTHOR_NAME:-Terra CI agent}"
git config --global user.email "${GIT_AUTHOR_EMAIL:-ci-agent@users.noreply.github.com}"
if [ ! -d /work/terra/.git ]; then
  git clone "https://github.com/$repo.git" /work/terra
fi
cd /work/terra
git fetch -q origin main
git checkout -q main
git merge -q --ff-only origin/main || echo "terra-ci: the checkout has commits origin/main lacks; left as it is"
if [ "$#" -gt 0 ]; then
  exec node tools/ci/ci.ts "$@"
fi
exec node tools/ci/ci.ts --watch "${TERRA_CI_EVERY:-2}"
