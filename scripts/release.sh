#!/usr/bin/env bash
# Build release bundles of the desktop app into dist/<version>/.
#
#   scripts/release.sh               # .deb, AppImage and Windows .exe installer
#   scripts/release.sh deb appimage  # only the Linux bundles
#   scripts/release.sh exe           # only the .exe (NSIS installer, cross-compiled)
#
# SHA256SUMS in the output directory lists every bundle there.
#
# With TAURI_SIGNING_PRIVATE_KEY set (the key, or a path to it), the AppImage and .exe also get
# the .sig file the in-app updater checks. Without it the bundles are built unsigned.
#
# The .exe is cross-compiled from Linux with cargo-xwin, which downloads the
# MSVC CRT and Windows SDK on first use (accepting Microsoft's license).
# Extra prerequisites for it:
#   rustup target add x86_64-pc-windows-msvc
#   cargo install --locked cargo-xwin
#   sudo apt-get install -y nsis lld llvm clang
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
app="$root/apps/tech-tree"
win_target="x86_64-pc-windows-msvc"

targets=("$@")
[[ ${#targets[@]} -eq 0 ]] && targets=(deb appimage exe)
for t in "${targets[@]}"; do
	[[ $t == deb || $t == appimage || $t == exe ]] ||
		{ echo "unknown target: $t (expected deb, appimage or exe)" >&2; exit 2; }
done

missing=()
need() { command -v "$1" >/dev/null || missing+=("$1 ($2)"); }
need pnpm "see docs/DEVELOPMENT.md"
need cargo "see docs/DEVELOPMENT.md"
for t in "${targets[@]}"; do
	case $t in
	deb) need dpkg-deb "apt-get install dpkg" ;;
	appimage) need file "apt-get install file" ;;
	exe)
		need cargo-xwin "cargo install --locked cargo-xwin"
		need makensis "apt-get install nsis"
		need lld-link "apt-get install lld"
		need llvm-rc "apt-get install llvm"
		need clang-cl "apt-get install clang"
		rustup target list --installed | grep -qx "$win_target" ||
			missing+=("Rust target $win_target (rustup target add $win_target)")
		;;
	esac
done
if [[ ${#missing[@]} -gt 0 ]]; then
	echo "missing prerequisites:" >&2
	printf '  - %s\n' "${missing[@]}" >&2
	exit 1
fi

# tauri.conf.json takes its version from package.json.
version="$(sed -n 's/^\s*"version": *"\([^"]*\)".*/\1/p' "$app/package.json" | head -n1)"
out="$root/dist/$version"
mkdir -p "$out"

cd "$app"
pnpm install --frozen-lockfile
pnpm build
# The frontend is built once above; skip tauri's beforeBuildCommand.
if [[ -n ${TAURI_SIGNING_PRIVATE_KEY:-} ]]; then
	no_frontend='{"build":{"beforeBuildCommand":""}}'
else
	echo "TAURI_SIGNING_PRIVATE_KEY is not set: building without updater signatures" >&2
	no_frontend='{"build":{"beforeBuildCommand":""},"bundle":{"createUpdaterArtifacts":false}}'
fi

# Copy bundles to $out, with their updater signatures when this run signs (the updater uses the
# AppImage and the .exe; a .deb has none). A signature left in $out or the build output by an
# earlier signed build would not match the new bundle.
copy_bundle() {
	for f in "$@"; do
		cp -v "$f" "$out/"
		rm -f "$out/$(basename "$f").sig"
		if [[ -n ${TAURI_SIGNING_PRIVATE_KEY:-} && $f != *.deb ]]; then
			[[ -f $f.sig && ! $f -nt $f.sig ]] ||
				{ echo "no fresh updater signature for $f" >&2; exit 1; }
			cp -v "$f.sig" "$out/"
		fi
	done
}

for t in "${targets[@]}"; do
	case $t in
	deb)
		pnpm tauri build --config "$no_frontend" --bundles deb
		copy_bundle "$root"/target/release/bundle/deb/*_"$version"_*.deb
		;;
	appimage)
		pnpm tauri build --config "$no_frontend" --bundles appimage
		copy_bundle "$root"/target/release/bundle/appimage/*_"$version"_*.AppImage
		;;
	exe)
		pnpm tauri build --config "$no_frontend" --runner cargo-xwin --target "$win_target" --bundles nsis
		copy_bundle "$root/target/$win_target/release/bundle/nsis/"*_"$version"_*-setup.exe
		;;
	esac
done

cd "$out"
shopt -s nullglob
sha256sum -- *.deb *.AppImage *.exe >SHA256SUMS

echo
echo "release bundles in $out:"
ls -1
