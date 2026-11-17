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

Browse the research tree, a wiki of game entities and a map of the solar system. More tools will follow.

## Install

Download the latest version from the [releases page](https://github.com/0xV1p3r/terra-invicta-tools/releases):

| System | Download |
| --- | --- |
| Windows 10 and 11 | `Terra Invicta Tools_<version>_x64-setup.exe` |
| Debian, Ubuntu | `terra-invicta-tools_<version>_amd64.deb`, then `sudo apt install ./terra-invicta-tools_<version>_amd64.deb` |
| Other Linux | `terra-invicta-tools_<version>_amd64.AppImage`. Make it executable and run it. Needs FUSE 2 (`libfuse2`). |

On Windows, SmartScreen may warn about an unknown publisher because the installer is not code-signed. Choose *More info*, then *Run anyway*.

`SHA256SUMS` on the release page lists the checksums of the downloads.

Need another platform? [Open an issue](https://github.com/0xV1p3r/terra-invicta-tools/issues).

## First run

You need Terra Invicta installed. The app finds it in your Steam libraries; if it can't, choose the game's install folder. Pick your extraction options and the app extracts the game data, then opens the tree.

Optional large artwork (illustrations, portraits and hab modules) takes about 1.5 GB.

## Game updates and app updates

- **Game updates:** the app tells you when a new game build is installed so you can extract it. Settings → Game data lets you extract again, change the data folder or delete old builds.
- **App updates:** the app checks GitHub for new versions at startup. Settings → Updates lets you check by hand or turn the check off. See [CHANGELOG.md](CHANGELOG.md) for what changed.

Extracted data lives in:

- Linux: `~/.local/share/com.terra-invicta-tools.desktop/data`
- Windows: `%APPDATA%\com.terra-invicta-tools.desktop\data`

## Development

The project is a Rust workspace (data extraction and querying) plus a Tauri 2 + SvelteKit desktop app. To run it from source:

```sh
cd apps/tech-tree
pnpm install --frozen-lockfile
pnpm app:dev
```

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for requirements, the extraction CLI, checks and the release process.

## License

[MIT](LICENSE).

This is a fan project with no connection to Pavonis Interactive or Hooded Horse. The game's data and images belong to its publisher and are not in this repository or the installers.
