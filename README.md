<h1 align="center"><img src="apps/tech-tree/branding/icon.svg" width="48" align="center" alt=""> Terra Invicta Tools</h1>

Unofficial desktop tools for Terra Invicta. The app reads data from your installed copy of the game and shows the research tree, a wiki and a solar system map. More tools will follow.

## Layout

| Path | Contents |
| --- | --- |
| `extractor/` | Python CLI that reads templates, localization and images from the game and writes one dataset per game build to `data/<buildId>/` |
| `crates/ti-data/` | Rust crate that loads and queries those datasets |
| `apps/tech-tree/` | The desktop app, built with Tauri 2 and SvelteKit |
| `docs/research/` | Notes on game rules and save files |

`data/` is gitignored, so extracted game data never leaves your machine.

## Requirements

- An installed copy of Terra Invicta
- Rust with rustfmt and clippy
- Node.js LTS and pnpm, at the version set in `apps/tech-tree/package.json`
- uv, which installs Python 3.12 or newer if you don't have it
- On Linux, the Tauri 2 build packages. On Ubuntu:

```sh
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev librsvg2-dev libayatana-appindicator3-dev patchelf
```

## Quick start

```sh
cd extractor
uv run ti-extract extract --data-dir ../data
cd ../apps/tech-tree
pnpm install --frozen-lockfile
pnpm app:dev
```

## Extract game data

Run these from `extractor/`. The extractor finds Steam installs by itself. To use another install, set `TI_GAME_DIR` or put `--game-dir PATH` before the subcommand.

```sh
uv run ti-extract info                         # show the detected install
uv run ti-extract extract --data-dir ../data   # write data/<buildId>/ and update data/index.json
uv run ti-extract diff ../data/OLD ../data/NEW --lang en -o changes.md
```

Options for `extract`:

| Option | Effect |
| --- | --- |
| `--force` | Replace an existing extraction of the same build |
| `--build-id ID` | Use `ID` as the output directory name |
| `--no-assets` | Skip images |
| `--skip-bundle NAME` | Skip one image bundle. You can repeat it. |
| `--image-format png\|webp` | Set the image format. PNG is the default. |

## Run the app

From `apps/tech-tree/`, `pnpm app:dev` starts the app and `pnpm app:build` builds installers.

The app takes the first data folder it finds in this order:

1. `TI_DATA_DIR`, if it is set and not empty
2. The folder you chose in the app's settings, if it still contains `index.json`
3. A `data/` folder with an `index.json`, searching upward from the working directory and then from the executable

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

From `apps/tech-tree/`, run `pnpm check`, `pnpm lint` and `pnpm test`. From `extractor/`, run `uv run pytest -q`.

## Legal

This is a fan project with no connection to Pavonis Interactive or Hooded Horse. The game's data and images belong to its publisher and are not in this repository.
