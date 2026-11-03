"""Locating the Terra Invicta install and discovering its data layers."""

from __future__ import annotations

import os
import re
from dataclasses import dataclass
from pathlib import Path

APP_ID = "1176470"
INSTALL_DIR_NAME = "Terra Invicta"
BASE_LAYER = "base"

_VDF_PAIR = re.compile(r'"([^"]+)"\s+"([^"]*)"')


def _steam_roots() -> list[Path]:
    home = Path.home()
    candidates = [
        home / ".local/share/Steam",
        home / ".steam/steam",
        home / ".var/app/com.valvesoftware.Steam/.local/share/Steam",
        Path("C:/Program Files (x86)/Steam"),
        home / "Library/Application Support/Steam",
    ]
    return [p for p in candidates if (p / "steamapps").is_dir()]


def _library_folders(steam_root: Path) -> list[Path]:
    libs = [steam_root]
    vdf = steam_root / "steamapps/libraryfolders.vdf"
    if vdf.is_file():
        for key, value in _VDF_PAIR.findall(vdf.read_text(encoding="utf-8", errors="replace")):
            if key == "path":
                libs.append(Path(value.replace("\\\\", "\\")))
    return libs


def find_game_dir() -> Path | None:
    for root in _steam_roots():
        for lib in _library_folders(root):
            candidate = lib / "steamapps/common" / INSTALL_DIR_NAME
            if (candidate / "TerraInvicta_Data").is_dir():
                return candidate
    return None


def read_app_manifest(game_dir: Path) -> dict[str, str]:
    """Top-level key/values of `appmanifest_<appid>.acf` next to the install."""
    acf = game_dir.parent.parent / f"appmanifest_{APP_ID}.acf"
    if not acf.is_file():
        return {}
    values: dict[str, str] = {}
    for key, value in _VDF_PAIR.findall(acf.read_text(encoding="utf-8", errors="replace")):
        values.setdefault(key, value)  # first occurrence = top level
    return values


def layer_id(relative_folder: Path) -> str:
    """Prefix the complete DLC-relative path, retaining a trailing Templates.

    `DarkSkies/2003_Scenario/Templates` -> `dlc.darkskies.2003_scenario.templates`.
    Keeping every directory component avoids conflating different source folders.
    """
    return "dlc." + relative_folder.as_posix().lower().replace("/", ".").replace(" ", "_")


_VERSION = re.compile(rb"\d+\.\d+\.\d+[a-z]?\d*")
_UNITY_VERSION = re.compile(rb"\d{4}\.\d+\.\d+[abfp]\d+")


def extract_version(data: bytes) -> str | None:
    """Find a length-prefixed bundleVersion, excluding the Unity engine version."""
    for match in _VERSION.finditer(data):
        start = match.start()
        value = match.group()
        if (start >= 4 and not _UNITY_VERSION.fullmatch(value)
                and int.from_bytes(data[start - 4:start], "little", signed=True) == len(value)):
            return value.decode("ascii")
    return None


def read_game_version(root: Path) -> str | None:
    path = root / "TerraInvicta_Data/globalgamemanagers"
    return extract_version(path.read_bytes()) if path.is_file() else None


@dataclass
class Layer:
    """One directory of templates, preserved without scenario resolution."""

    id: str
    kind: str  # "base" | "dlc"
    templates_dir: Path
    dlc: str | None = None
    folder: str | None = None


@dataclass
class GameInstall:
    root: Path
    version: str | None
    build_id: str | None
    last_updated: int | None
    layers: list[Layer]
    bundle_dirs: list[Path]

    @property
    def streaming_assets(self) -> Path:
        return self.root / "TerraInvicta_Data/StreamingAssets"

    @property
    def languages(self) -> list[str]:
        loc_root = self.streaming_assets / "Localization"
        return sorted(p.name for p in loc_root.iterdir() if p.is_dir()) if loc_root.is_dir() else []


def discover(root: Path) -> GameInstall:
    sa = root / "TerraInvicta_Data/StreamingAssets"
    if not (sa / "Templates").is_dir():
        raise FileNotFoundError(f"no Templates directory under {sa}")

    base = Layer(BASE_LAYER, "base", sa / "Templates")
    layers = [base]
    bundle_dirs = [sa / "AssetBundles"]

    dlc_root = root / "DLC_Content"
    if dlc_root.is_dir():
        for dlc in sorted(p for p in dlc_root.iterdir() if p.is_dir()):
            if (dlc / "AssetBundles").is_dir():
                bundle_dirs.append(dlc / "AssetBundles")
        directories = {p.parent for p in dlc_root.rglob("*.json") if p.is_file()}
        for directory in sorted(directories, key=lambda p: p.relative_to(dlc_root).as_posix()):
            relative = directory.relative_to(dlc_root)
            layers.append(Layer(layer_id(relative), "dlc", directory,
                                dlc=relative.parts[0], folder=relative.as_posix()))

    ids = [layer.id for layer in layers]
    if len(set(ids)) != len(ids):
        raise ValueError(f"layer id collision: {ids}")

    manifest = read_app_manifest(root)
    last_updated = manifest.get("LastUpdated")
    return GameInstall(
        root=root,
        version=read_game_version(root),
        build_id=manifest.get("buildid"),
        last_updated=int(last_updated) if last_updated and last_updated.isdigit() else None,
        layers=layers,
        bundle_dirs=[d for d in bundle_dirs if d.is_dir()],
    )


def resolve_game_dir(explicit: str | None) -> Path:
    if explicit:
        return Path(os.path.expanduser(explicit))
    env = os.environ.get("TI_GAME_DIR")
    if env:
        return Path(os.path.expanduser(env))
    found = find_game_dir()
    if not found:
        raise SystemExit("Terra Invicta install not found; pass --game-dir or set TI_GAME_DIR")
    return found
