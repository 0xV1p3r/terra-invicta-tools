# Terra Invicta tools

Tools for exploring an installed copy of Terra Invicta. The Python extractor reads game templates, localization and images into build-specific datasets. The Rust `ti-data` crate queries those datasets, and the Tauri desktop app displays the research tree, wiki and solar system.

The pipeline is `extractor/` → `data/<buildId>/` → `crates/ti-data/` → `apps/tech-tree/`. Extracted data stays local and is gitignored.

## Prerequisites

Install a Rust toolchain with rustfmt and clippy, pnpm at the version in `apps/tech-tree/package.json`, Node.js LTS, and uv. The extractor requires Python 3.12 or newer; uv can provision it. You need an installed copy of the game to extract data.

On Ubuntu, install Tauri 2's Linux build dependencies:

```sh
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev librsvg2-dev libayatana-appindicator3-dev patchelf
```

## Extract data

From the repository root:

```sh
cd extractor
uv run ti-extract info
uv run ti-extract extract --data-dir ../data
```

The extractor detects Steam installs, or reads `TI_GAME_DIR`. To specify an install explicitly, put the global option before the subcommand:

```sh
uv run ti-extract --game-dir /path/to/TerraInvicta extract --data-dir ../data
```

Extraction writes `data/<buildId>/` and updates `data/index.json`. Use `--force` to replace an existing extraction, or `--build-id ID` to override its directory name. `--no-assets` skips images, `--skip-bundle NAME` skips a named image bundle and can be repeated, and `--image-format png` or `--image-format webp` selects the image format. PNG is the default.

Compare two extracted builds from `extractor/`:

```sh
uv run ti-extract diff ../data/OLD_BUILD ../data/NEW_BUILD --lang en -o changes.md
```

## Run the desktop app

```sh
cd apps/tech-tree
pnpm install --frozen-lockfile
pnpm app:dev
# Build the desktop app and bundles:
pnpm app:build
```

The app looks for data in this order: a nonempty `TI_DATA_DIR`, the folder chosen in the app and saved in `<app config dir>/settings.json` if it still contains `index.json`, then a `data/` directory containing `index.json` above the current working directory or executable. Point it at the data root, not an individual build folder. The app can remember a selected build; otherwise it uses the latest.

For browser development, generate fixtures from local extracted data at the repository root:

```sh
# Tauri's configured frontendDist must exist to compile the app crate.
mkdir -p apps/tech-tree/build
cargo test -p ti-tech-tree --release -- --ignored write_fixtures
cd apps/tech-tree
pnpm dev
```

Outside Tauri, the API uses JSON fixtures in `apps/tech-tree/static/mock/`. These files are gitignored. Fixtures use overview scope and English, ignore scope and edge merging, and only cover representative entities and searches. Missing fixtures return errors. Folder and build switching are unavailable in mock mode. See `src-tauri/src/fixtures.rs` and the mock section of `src/lib/api.ts` for coverage.

## Checks before a change

From the repository root:

```sh
# Tauri's configured frontendDist must exist for builds embedding assets.
mkdir -p apps/tech-tree/build
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p ti-tech-tree export_bindings
git diff --exit-code -- apps/tech-tree/src/lib/generated
```

Commit regenerated bindings alongside DTO changes. Rust tests that require real game data skip themselves when data is absent.

From `apps/tech-tree/`:

```sh
pnpm check
pnpm lint
pnpm test
```

From `extractor/`:

```sh
uv run pytest -q
```

See `docs/research/` for notes on game semantics. Game data and images are copyrighted by the game's publisher and are not part of this repository.
