# The render worker's container (docs/render-worker.md, D445): the CI agent's toolchain
# (Dockerfile) with what a render needs on an NVIDIA card: the Vulkan loader and the libraries
# Bevy and wgpu load, and ffmpeg for films. The driver itself, its Vulkan ICD included, is put
# in by the NVIDIA Container Toolkit when the container starts (compose.yaml asks for the GPU).
# The repository is cloned into the /work volume on first start, as the CI agent's is.

FROM node:22.18-bookworm-slim AS node

# The compiler rust-toolchain.toml pins.
FROM rust:1.97.1-bookworm

RUN apt-get update \
 && apt-get install --no-install-recommends -y pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev \
      libvulkan1 vulkan-tools libegl1 libgl1 libx11-6 libxext6 libxcursor1 libxrandr2 libxi6 libxkbcommon-x11-0 \
      ffmpeg \
 && rm -rf /var/lib/apt/lists/*

COPY --from=node /usr/local/bin/node /usr/local/bin/node

# gh from its release, the version pinned (Debian's is years behind).
ARG GH_VERSION=2.102.0
ARG TARGETARCH
RUN curl -fsSL "https://github.com/cli/cli/releases/download/v${GH_VERSION}/gh_${GH_VERSION}_linux_${TARGETARCH:-amd64}.tar.gz" \
  | tar -xz -C /tmp \
 && mv "/tmp/gh_${GH_VERSION}_linux_${TARGETARCH:-amd64}/bin/gh" /usr/local/bin/gh \
 && rm -rf /tmp/gh_*

# Every driver capability: graphics brings the Vulkan ICD, utility nvidia-smi, video NVENC.
ENV NVIDIA_VISIBLE_DEVICES=all NVIDIA_DRIVER_CAPABILITIES=all

COPY render-entrypoint.sh /usr/local/bin/terra-render
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/terra-render"]
