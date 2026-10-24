#!/bin/sh
# The render worker container's start (docs/render-worker.md): sign git in with the token, make
# sure Vulkan can find the NVIDIA driver, say which card it sees, clone the repository into the
# /work volume the first time, then watch for jobs. Arguments, if any, go to the worker instead
# (`docker compose run --rm render --plan`).
set -eu
: "${GH_TOKEN:?set GH_TOKEN in ~/terra-ci/.env (see tools/render-worker/README.md)}"
repo="${TERRA_REPO:-dcramer/terra}"
gh auth setup-git
git config --global user.name "${GIT_AUTHOR_NAME:-Terra render worker}"
git config --global user.email "${GIT_AUTHOR_EMAIL:-render-worker@users.noreply.github.com}"
# Some toolkit versions mount the driver's Vulkan ICD file only under /etc, or not at all, and
# the loader then finds no device: point it at the driver's library the toolkit mounted.
if [ ! -e /etc/vulkan/icd.d/nvidia_icd.json ] && [ ! -e /usr/share/vulkan/icd.d/nvidia_icd.json ]; then
  mkdir -p /usr/share/vulkan/icd.d
  printf '{"file_format_version":"1.0.0","ICD":{"library_path":"libGLX_nvidia.so.0","api_version":"1.3"}}\n' > /usr/share/vulkan/icd.d/nvidia_icd.json
fi
if command -v nvidia-smi >/dev/null 2>&1; then
  echo "terra-render: $(nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader | head -1)"
else
  echo "✗ terra-render: no NVIDIA card in the container; install the NVIDIA Container Toolkit (tools/render-worker/README.md)"
fi
device="$(vulkaninfo --summary 2>/dev/null | grep -m1 deviceName || true)"
echo "terra-render: Vulkan: ${device:-✗ no device}"
if [ ! -d /work/terra/.git ]; then
  git clone "https://github.com/$repo.git" /work/terra
fi
cd /work/terra
git fetch -q origin main
git checkout -q main
git merge -q --ff-only origin/main || echo "terra-render: the checkout has commits origin/main lacks; left as it is"
if [ "$#" -gt 0 ]; then
  exec node tools/render-worker/worker.ts "$@"
fi
exec node tools/render-worker/worker.ts --watch "${TERRA_RENDER_EVERY:-30}"
