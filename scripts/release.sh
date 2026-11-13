#!/usr/bin/env bash
# Build release bundles of the desktop app into dist/<version>/.
#
#   scripts/release.sh            # .deb and Windows .exe installer
#   scripts/release.sh deb        # only the .deb
#   scripts/release.sh exe        # only the .exe (NSIS installer, cross-compiled)
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
[[ ${#targets[@]} -eq 0 ]] && targets=(deb exe)
for t in "${targets[@]}"; do
	[[ $t == deb || $t == exe ]] || { echo "unknown target: $t (expected deb or exe)" >&2; exit 2; }
done

missing=()
need() { command -v "$1" >/dev/null || missing+=("$1 ($2)"); }
need pnpm "see README requirements"
need cargo "see README requirements"
for t in "${targets[@]}"; do
	case $t in
	deb) need dpkg-deb "apt-get install dpkg" ;;
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

version="$(sed -n 's/^\s*"version": *"\([^"]*\)".*/\1/p' "$app/src-tauri/tauri.conf.json" | head -n1)"
out="$root/dist/$version"
mkdir -p "$out"

cd "$app"
pnpm install --frozen-lockfile
pnpm build
# The frontend is built once above; skip tauri's beforeBuildCommand.
no_frontend='{"build":{"beforeBuildCommand":""}}'

for t in "${targets[@]}"; do
	case $t in
	deb)
		pnpm tauri build --config "$no_frontend" --bundles deb
		cp -v "$root"/target/release/bundle/deb/*_"$version"_*.deb "$out/"
		;;
	exe)
		pnpm tauri build --config "$no_frontend" --runner cargo-xwin --target "$win_target" --bundles nsis
		cp -v "$root/target/$win_target/release/bundle/nsis/"*_"$version"_*-setup.exe "$out/"
		;;
	esac
done

echo
echo "release bundles in $out:"
ls -1 "$out"
