# The CI agent's container (docs/ci-agent.md): what `cargo xtask nightly` needs to build and test
# Terra headless, and `gh`. The repository itself is cloned into the /work volume when the
# container first starts, so the image changes only when the toolchain does.
# Build and run from tools/ci: `docker compose up -d --build`.

FROM node:22.18-bookworm-slim AS node

# The compiler rust-toolchain.toml pins.
FROM rust:1.97.1-bookworm

# What Bevy's crates link against on Linux, as .github/workflows/ci.yml installs.
RUN apt-get update \
 && apt-get install --no-install-recommends -y pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev \
 && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy

COPY --from=node /usr/local/bin/node /usr/local/bin/node

# gh from its release, the version pinned (Debian's is years behind).
ARG GH_VERSION=2.102.0
ARG TARGETARCH
RUN curl -fsSL "https://github.com/cli/cli/releases/download/v${GH_VERSION}/gh_${GH_VERSION}_linux_${TARGETARCH:-amd64}.tar.gz" \
  | tar -xz -C /tmp \
 && mv "/tmp/gh_${GH_VERSION}_linux_${TARGETARCH:-amd64}/bin/gh" /usr/local/bin/gh \
 && rm -rf /tmp/gh_*

COPY entrypoint.sh /usr/local/bin/terra-ci
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/terra-ci"]
