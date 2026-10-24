# tools/render-worker

The render worker ([docs/render-worker.md](../../docs/render-worker.md), D445): agents queue renders as draft GitHub releases, a GPU box runs them at a pushed commit and uploads what they wrote.

```bash
cargo xtask remote-render -- cargo xtask trailer --cut tools/sandbox/trailer/cut.json   # HEAD, which must be pushed
cargo xtask remote-render --rev 1a2b3c4 --collect shot.png -- cargo xtask sandbox --offscreen --experiment play --shot shot.png
cargo xtask remote-render --no-wait -- cargo xtask bench        # queue, print the job, return
cargo xtask remote-render --wait <job>                          # wait for it and download
cargo xtask remote-render --status                              # the worker's last word and every job
cargo xtask remote-render --local --no-cook -- <command>        # the whole flow on this machine: no GitHub, no box
```

- **Files back**: what the command wrote under `~/.cache/terra-review/` while it ran (a film's `frames/` folders left out), or the `--collect` paths instead (in the checkout, or `~/…`), unpacked into `~/.cache/terra-review/remote/<job>/` beside `job.log`. Output: a line when the job's state changes, the folder at the end; on a failure the log's last lines. Exit code: the command's (124 at its `--minutes` limit, default 120).
- **Files**: `client.ts` is the xtask, `worker.ts` the worker its container runs (`--watch [seconds]`, `--once`, `--plan`), `store.ts` the job store (GitHub's draft releases through `gh`, or a folder for `--local` and the tests), `jobs.ts` the decisions as pure functions (`jobs.test.ts`); `flow.test.ts` runs the whole path on a scratch repository in under a second. Node 22.18 or later, git, gh, tar; no dependencies.
- **The container** lives with the CI agent's in [tools/ci](../ci/README.md): `render.Dockerfile`, `render-entrypoint.sh`, and the `render` service of `compose.yaml`, which `up.sh` starts when `TERRA_RENDER=1`. The worker's checkout and build are in its `render` volume (`/work/terra/.git/terra-render/src`), its caches (cooked textures, shaders, the review folder) in `render-cache`.

## Setting up the box

The box is the CI agent's (docs/ci-agent.md) with an NVIDIA card, Linux. By hand, once:

1. **The driver.** `nvidia-smi` on the box prints the card. On Ubuntu: `sudo ubuntu-drivers install`, then reboot.
2. **Docker**, as for the CI agent: [docs.docker.com/engine/install](https://docs.docker.com/engine/install/), and `sudo usermod -aG docker $USER`, then log in again.
3. **The NVIDIA Container Toolkit**, following [NVIDIA's install guide](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html) for the box's distribution (on Ubuntu: add NVIDIA's apt repository, `sudo apt-get install -y nvidia-container-toolkit`), then `sudo nvidia-ctk runtime configure --runtime=docker && sudo systemctl restart docker`. Check: `docker run --rm --gpus all ubuntu nvidia-smi` prints the card.
4. **The token**: the CI agent's, a classic GitHub token with only the `public_repo` scope, from an account with push access (draft releases need it). It stays in `~/terra-ci/.env` on the box and nowhere else.
5. **Start it**, over SSH on the box:

   ```bash
   TERRA_RENDER=1 GH_TOKEN=<token> sh -c 'curl -fsSL https://raw.githubusercontent.com/dcramer/terra/main/tools/ci/up.sh | sh'
   ```

   Leave out `GH_TOKEN=<token>` when the CI agent is already set up. It ends with the card the container sees. The first job builds the game, which takes a while; later ones build only what changed.

6. **Optional**, in `~/terra-ci/.env`, then run the command again: `TERRA_RENDER_WINDOW=22:00-08:00` (jobs start only in those hours), `TZ=Europe/Warsaw` (the window's clock), `TERRA_RENDER_AFTER_CI=0` (start jobs while nightly runs too), `TERRA_RENDER_NAME` (the box's name in job notes), `TERRA_RENDER_EVERY` (seconds between looks, default 30). `TERRA_RENDER=0` takes the worker off again.

Disk: about 100 GB more than the CI agent's, for the worker's two clones, its build and its caches.

**Check it**: `docker compose -f ~/terra-ci/compose.yaml logs render` starts with the card and the Vulkan device it sees; `cargo xtask remote-render --status` from any checkout says when the worker was last seen. `docker compose -f ~/terra-ci/compose.yaml run --rm render --plan` prints what it would do next.

## Not yet verified

Built and tested without the box: the flow end to end with a folder for a store (`flow.test.ts`, and `--local` on the real repository), the compose file's shape. Not yet run: the image's build, Vulkan and Bevy offscreen inside the container, the GitHub store against real draft releases, the game's sound with no sound device in a trailer job. The first job on the box checks them; D445 stays Leaning until then.
