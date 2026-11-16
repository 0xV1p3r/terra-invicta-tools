<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="apps/tech-tree/branding/wordmark-dark.svg">
    <img src="apps/tech-tree/branding/wordmark-light.svg" alt="Terra Invicta Tools" width="380">
  </picture>
</p>

<p align="center">
  Unofficial desktop tools for Terra Invicta, built from the data in your installed copy of the game.
</p>

---

The app shows the research tree, a wiki and a solar system map. More tools will follow.

## Install

Download the latest version from the [releases page](https://github.com/0xV1p3r/terra-invicta-tools/releases):

| System | Download |
| --- | --- |
| Windows 10 and 11 | `Terra Invicta Tools_<version>_x64-setup.exe` |
| Debian, Ubuntu | `terra-invicta-tools_<version>_amd64.deb`, installed with `sudo apt install ./terra-invicta-tools_<version>_amd64.deb` |
| Other Linux distributions | `terra-invicta-tools_<version>_amd64.AppImage`. Make it executable and run it. It needs FUSE 2 (`libfuse2`). |

The Windows installer is not code-signed, so SmartScreen may warn about an unknown publisher. Choose *More info*, then *Run anyway*. `SHA256SUMS` lists the checksums of the downloads. [CHANGELOG.md](CHANGELOG.md) lists the changes in each version.

## Layout

| Path | Contents |
| --- | --- |
| `crates/ti-extract/` | Rust library and CLI that extract one dataset per game build to `data/<buildId>/` |
| `crates/ti-unity/` | Reads images from the game's Unity asset bundles |
| `crates/ti-data/` | Rust crate that loads and queries those datasets |
| `apps/tech-tree/` | The desktop app, built with Tauri 2 and SvelteKit |
| `docs/research/` | Notes on game rules and save files |

No game data is in the repo or the installers. It is built locally from your own game files. `data/` is gitignored.

## First run

Open the installed app. It finds Terra Invicta in your Steam libraries, lets you choose extraction options, then extracts the data and opens the tree. If it cannot find the game, choose its install folder.

Data is stored in the app's data folder: `~/.local/share/dev.terra-invicta-tools.tech-tree/data` on Linux, `%APPDATA%\dev.terra-invicta-tools.tech-tree\data` on Windows. Large artwork (illustrations, portraits and hab modules, about 1.5 GB) is optional. Lossless WebP is the default: same quality as PNG, faster to extract and smaller.

The app shows a notice when the game updates so you can extract the new build. In Settings → Game data, you can extract again, choose a data folder or delete builds you no longer need.

## Requirements

- An installed copy of Terra Invicta
- Rust with rustfmt and clippy
- Node.js LTS and pnpm, at the version set in `apps/tech-tree/package.json`
- On Linux, the Tauri 2 build packages. On Ubuntu:

```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev librsvg2-dev libayatana-appindicator3-dev patchelf
```

## Quick start

For developers, from the repository root:

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

## Build release bundles

`scripts/release.sh` writes a `.deb`, an AppImage and a Windows installer (`-setup.exe`) to `dist/<version>/`, with their checksums in `SHA256SUMS`. Pass `deb`, `appimage` or `exe` to build only some of them. The bundles do not contain game data. The version is set in `apps/tech-tree/package.json`, which `tauri.conf.json` reads.

An AppImage contains the system libraries of the machine that built it, so it only runs on distributions at least as new. Release AppImages are built on Ubuntu 22.04 by CI.

The Windows installer is cross-compiled on Linux with cargo-xwin. The first build downloads the MSVC CRT and Windows SDK. It needs:

```sh
rustup target add x86_64-pc-windows-msvc
cargo install --locked cargo-xwin
sudo apt-get install -y nsis lld llvm clang
```

### Publish a release

1. Set the version in `apps/tech-tree/package.json` and move its notes in `CHANGELOG.md` under `## [<version>] - <date>`.
2. Push a tag `v<version>` to the commit. The `Release` workflow builds the `.deb` and AppImage on Ubuntu 22.04 and the installer on Windows, then opens a draft GitHub release with the bundles, `SHA256SUMS` and that section of `CHANGELOG.md` as notes.
3. Check the draft and publish it.

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

CI (`.github/workflows/ci.yml`) runs all of these on pull requests and on pushes to `main`.

## License

[MIT](LICENSE).

This is a fan project with no connection to Pavonis Interactive or Hooded Horse. The game's data and images belong to its publisher and are not in this repository.
