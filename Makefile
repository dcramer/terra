# The front door for a new machine: `make setup` once, then `make play` (D387).
#
# Every real task is `cargo xtask <task>` (`make tools` lists them all). This file only installs
# what Cargo cannot install for itself (rustup, the pinned compiler, the system's libraries) and
# names the everyday tasks, so nobody needs to know Cargo to start. Pass a task's own options
# with ARGS, as in `make play ARGS="--time 18:00"`.

SHELL := /bin/bash
.SHELLFLAGS := -eu -o pipefail -c
# rustup installs cargo here, and a fresh install is not on PATH until a new shell.
export PATH := $(HOME)/.cargo/bin:$(PATH)

UNAME := $(shell uname -s)
WSL := $(shell grep -qi microsoft /proc/version 2>/dev/null && echo yes)
ARGS ?=

.DEFAULT_GOAL := help
.PHONY: help setup system rust node hooks build update play fresh creative yard doctor tools check quick test fmt bench clean

help: ## List these targets
	@echo "Terra. On a new machine: make setup, then make play."
	@echo
	@grep -hE '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  make %-8s %s\n", $$1, $$2}'
	@echo
	@echo "Pass a task's options with ARGS, e.g. make play ARGS=\"--build debug --time 18:00\" (any skips the menu)."
	@echo "Every other task: make tools."

# --- Getting a machine ready ----------------------------------------------------------------

setup: system rust node hooks build doctor ## Install what is missing, build the game and check the machine
	@echo
	@echo "Ready. Play with: make play"

# Xcode's command line tools on macOS, Bevy's libraries on Linux (part of setup).
system:
ifeq ($(UNAME),Darwin)
	@xcode-select -p >/dev/null 2>&1 || { \
	  echo "Installing Xcode's command line tools. Finish the dialog that opens, then run make setup again."; \
	  xcode-select --install || true; exit 1; }
else ifeq ($(UNAME),Linux)
	@pkgs="pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev$(if $(WSL), mingw-w64)"; \
	if command -v dpkg-query >/dev/null; then \
	  missing=$$(for p in $$pkgs; do dpkg-query -W -f='$${Status}' $$p 2>/dev/null | grep -q 'ok installed' || echo $$p; done | xargs); \
	  if [ -n "$$missing" ]; then echo "Installing $$missing (asks for your password)"; sudo apt-get install -y $$missing; fi; \
	else \
	  echo "Not Debian or Ubuntu: install pkg-config and the alsa, udev, wayland and xkbcommon development packages yourself."; \
	fi
endif

# Rustup, and the compiler pinned in rust-toolchain.toml (part of setup).
rust:
	@command -v rustup >/dev/null || { \
	  echo "Installing rustup (https://rustup.rs)"; \
	  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none; }
	@# With no name, rustup reads rust-toolchain.toml: the compiler, rustfmt and clippy.
	@rustup toolchain install || rustup show active-toolchain
ifeq ($(WSL),yes)
	@rustup target add x86_64-pc-windows-gnu
endif

# Check for Node 22.18 or later, which runs the TypeScript tools (part of setup).
node:
	@node -e 'const [a, b] = process.versions.node.split(".").map(Number); process.exit(a > 22 || (a === 22 && b >= 18) ? 0 : 1)' 2>/dev/null || { \
	  echo "The tools need Node 22.18 or later (found: $$(node --version 2>/dev/null || echo none))."; \
	  echo "  Install it with: brew install node   (or: nvm install 22)"; \
	  echo "  The game builds and plays without it."; }

# Git runs the formatter before each commit and the formatter and clippy before each push (part of setup).
hooks:
	@git config core.hooksPath .githooks

build: ## Build the game, without starting it (the first build takes a few minutes)
	cargo build -p terra_sandbox

update: ## Get the latest from GitHub, then build it
	git pull --ff-only
	@$(MAKE) --no-print-directory build

# --- Everyday --------------------------------------------------------------------------------

play: ## The launcher: continue, a new game, creative or the yard, and the build; Enter plays the last
	cargo xtask play $(ARGS)

fresh: ## Play a new game, replacing your saved one (no menu)
	cargo xtask play new $(ARGS)

creative: ## Play your world with building free (the stand-in for creative mode, D494)
	cargo xtask play creative $(ARGS)

yard: ## Play the yard: a flat level for building, free and instant, nothing saved
	cargo xtask play yard $(ARGS)

doctor: ## Say what this machine is missing, with the command that fixes each
	cargo xtask doctor

tools: ## List every cargo xtask task
	cargo xtask tools

quick: ## Formatting, as git checks before a commit
	cargo fmt --all --check

check: ## Formatting and clippy of what changed, as git runs before a push (seconds)
	cargo xtask check

test: ## Every test
	cargo test --workspace

fmt: ## Format the code
	cargo fmt --all

bench: ## Time the benchmark scenes on this machine's GPU
	cargo xtask bench $(ARGS)

clean: ## Delete the build (10 GB or more; the next build starts over)
	cargo clean
