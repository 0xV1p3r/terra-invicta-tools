# Development

## Layout

| Path | Contents |
| --- | --- |
| `crates/ti-extract/` | Rust library and CLI that extract one dataset per game build to `data/<buildId>/` |
| `crates/ti-unity/` | Reads images from the game's Unity asset bundles |
| `crates/ti-data/` | Rust crate that loads and queries those datasets |
| `apps/tech-tree/` | The desktop app, built with Tauri 2 and SvelteKit |
| `docs/research/` | Notes on game rules and save files |

No game data is in the repo or the installers. It is built locally from your own game files. `data/` is gitignored.

## Requirements

- An installed copy of Terra Invicta
- Rust with rustfmt and clippy
- Node.js LTS and pnpm, at the version set in `apps/tech-tree/package.json`
- On Linux, the Tauri 2 build packages. On Ubuntu:

```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev librsvg2-dev libayatana-appindicator3-dev patchelf
```

## Quick start

From the repository root:

```sh
cd apps/tech-tree
pnpm install --frozen-lockfile
pnpm app:dev
```

On first launch, the app extracts the game data. Alternatively, run this from the repository root to create a local `data/` folder that dev builds find by searching upward:

```sh
cargo run --release -p ti-extract -- extract --data-dir data
```

## Extract game data

Run these from the repository root. The Rust CLI finds Steam installs by itself. To use another install, set `TI_GAME_DIR` or put `--game-dir PATH` before the subcommand.

```sh
cargo run --release -p ti-extract -- info
cargo run --release -p ti-extract -- extract --data-dir data
cargo run --release -p ti-extract -- diff data/OLD data/NEW --lang en -o changes.md
```

`extract` writes `data/<buildId>/` and updates `data/index.json`.

Options for `extract`:

| Option | Effect |
| --- | --- |
| `--data-dir PATH` | Data root. Defaults to `data`. |
| `--force` | Replace an existing extraction of the same build |
| `--build-id ID` | Use `ID` as the output directory name |
| `--no-assets` | Skip images |
| `--skip-bundle NAME` | Skip one image bundle. You can repeat it. |
| `--image-format webp\|png` | Set the image format. Lossless WebP is the default. |
| `--allow-parse-errors` | Exit successfully even if template files fail to parse |

## Run the app

From `apps/tech-tree/`, `pnpm app:dev` starts the app and `pnpm app:build` builds installers.

The app takes the first data folder it finds in this order:

1. `TI_DATA_DIR`, if it is set and not empty
2. The folder you chose in the app's settings, if it still contains `index.json`
3. `<app data dir>/data`, if it contains `index.json`
4. A `data/` folder with an `index.json`, searching upward from the working directory and then from the executable

Give it the data root, not a single build folder. It opens the latest build unless you pick another one in settings.

### Browser development

`pnpm dev` runs the UI in a browser without Tauri. It reads JSON fixtures from `apps/tech-tree/static/mock/`, which you generate from your local data:

```sh
mkdir -p apps/tech-tree/build   # the app crate won't compile without it
cargo test -p ti-tech-tree --release -- --ignored write_fixtures
cd apps/tech-tree && pnpm dev
```

The fixtures cover a sample of entities and searches in English. Anything else returns an error, and you can't switch folders or builds in this mode. `src-tauri/src/fixtures.rs` lists what the fixtures include.

## Checks

From the repository root:

```sh
mkdir -p apps/tech-tree/build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p ti-tech-tree export_bindings
git diff --exit-code -- apps/tech-tree/src/lib/generated
```

If you change a DTO, commit the regenerated TypeScript bindings with it. Rust tests that need game data skip when `data/` is empty.

From `apps/tech-tree/`, run `pnpm check`, `pnpm lint` and `pnpm test`.

CI (`.github/workflows/ci.yml`) runs all of these on pull requests and on pushes to `main`, with the Rust toolchain pinned in `rust-toolchain.toml`.

### Workflow actions

`.github/workflows/actions.lock` records the commit each action in the workflows runs, and CI fails when it is out of date. After changing a `uses:` line, regenerate it with the [`gh actions-lock`](https://github.com/github/gh-actions-lock) extension and commit it:

```sh
gh extension install github/gh-actions-lock
gh actions-lock
```

Dependabot updates actions, the lockfile, the Rust toolchain and dependencies weekly, a week after each new version comes out.

## Build release bundles

`scripts/release.sh` writes a `.deb`, an AppImage and a Windows installer (`-setup.exe`) to `dist/<version>/`, with their checksums in `SHA256SUMS`. Pass `deb`, `appimage` or `exe` to build only some of them. The bundles do not contain game data. The version is set in `apps/tech-tree/package.json`, which `tauri.conf.json` reads.

An AppImage contains the system libraries of the machine that built it, so it only runs on distributions at least as new. Release AppImages are built on Ubuntu 22.04 by CI.

The Windows installer is cross-compiled on Linux with cargo-xwin. The first build downloads the MSVC CRT and Windows SDK. It needs:

```sh
rustup target add x86_64-pc-windows-msvc
cargo install --locked cargo-xwin
sudo apt-get install -y nsis lld llvm clang
```

### Updater signing

The in-app updater only installs bundles signed with the release key. Its public key is in `tauri.conf.json` (`plugins.updater.pubkey`); the private key is the `TAURI_SIGNING_PRIVATE_KEY` secret of the repository's `release` environment, which only `v*` tags can use. Keep a backup of the private key: if it is lost, installed copies can no longer be updated and have to be reinstalled by hand.

To sign local builds, set `TAURI_SIGNING_PRIVATE_KEY` to the key or a path to it (and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if it has one) before running `scripts/release.sh`. Without it, the bundles are built without signatures.

### Publish a release

1. Set the version in `apps/tech-tree/package.json` and move its notes in `CHANGELOG.md` under `## [<version>] - <date>`.
2. Push a tag `v<version>` to the commit. The `Release` workflow builds the `.deb` and AppImage on Ubuntu 22.04 and the installer on Windows, without the key. The `sign` job then signs them, and the `release` job attests their build provenance and opens a draft GitHub release with the bundles, their signatures, `latest.json` (the updater's manifest), `SHA256SUMS` and that section of `CHANGELOG.md` as notes.
3. Check the draft and publish it. Installed copies see the update once it is the latest published release.
