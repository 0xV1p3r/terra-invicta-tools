# Changelog

## [0.1.0] - unreleased

The first release. Terra Invicta Tools reads the data of your installed copy of Terra Invicta and shows:

- **Research tree**: every tech and engineering project with its prerequisites, filters by category, scenario, faction and property, search, a minimap and a zoomed-out overview.
- **Wiki**: game entities with their stats and descriptions, spoilers hidden unless you turn them on.
- **Solar system map**.

All 14 game languages are supported.

### Installing

- **Windows**: run `Terra Invicta Tools_0.1.0_x64-setup.exe`. The installer is not code-signed, so Windows SmartScreen may warn about an unknown publisher: choose _More info_ → _Run anyway_. If the Microsoft Edge WebView2 runtime is missing (older Windows 10), the installer downloads it.
- **Debian, Ubuntu**: `sudo apt install ./terra-invicta-tools_0.1.0_amd64.deb`
- **Other Linux distributions**: make `terra-invicta-tools_0.1.0_amd64.AppImage` executable and run it. AppImages need FUSE 2 (`libfuse2`, or `libfuse2t64` on Ubuntu 24.04 and later).

`SHA256SUMS` lists the checksums of the downloads.

### First run

The app finds Terra Invicta in your Steam libraries (or asks for its folder) and extracts the game data, which takes a few minutes. The data takes about 120 MB; the optional large artwork adds about 1.5 GB. Nothing is sent anywhere: the app works offline.

When the game updates, the app offers to extract the new build.

### Updates

At startup the app asks GitHub whether a newer version exists and, if so, offers to install it and restart. Downloads are checked against the release signing key before they are installed. Updating a `.deb` install asks for your administrator password. You can turn the check off, or check by hand, in Settings → Updates.

### Known limitations

- Mods are not read.
- No macOS build.
