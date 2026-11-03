"""Game-compatible line parsing and one merged dictionary per language.

Base files load by name with first-key-wins diagnostics. All DLC language
files then load by relative path and replace earlier values. Mods are excluded.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .output import sha256_file


@dataclass
class LocaleSet:
    # lang -> key -> text
    entries: dict[str, dict[str, str]] = field(default_factory=dict)
    # relative source path -> sha256
    sources: dict[str, str] = field(default_factory=dict)
    issues: list[dict[str, Any]] = field(default_factory=list)


def parse_line(line: str) -> tuple[str, str] | None:
    line = line.rstrip("\r\n")
    if not line.strip():
        return None
    separator = line.find("=")
    if separator < 1:
        return None
    key, value = line[:separator], line[separator + 1:]
    value = (value.replace("\t", "")
             .replace("<br/>", "\n")
             .replace("<h>", "<color=#EC9933>")
             .replace("</h>", "</color>")
             .replace("<rcol>", '<line-height=0.01%>\n<align="right">')
             .replace("</rcol>", "</align></line-height>"))
    value = value.split("//", 1)[0].rstrip(" ").replace("<sp>", " ")
    if not value:
        return None
    return key, value.replace("<skip/>", "")


def parse_file(path: Path, issues: list, *, lang: str, layer: str,
               entries: dict[str, str] | None = None) -> dict[str, str]:
    """Load into an existing dictionary, preserving base keys across files."""
    if entries is None:
        entries = {}
    text = path.read_text(encoding="utf-8-sig", errors="replace")
    # File.ReadLines splits CR/LF, not Unicode separators or vertical tabs.
    for lineno, line in enumerate(text.split("\n"), 1):
        parsed = parse_line(line)
        if parsed is None:
            continue
        key, value = parsed
        if layer == "base" and key in entries:
            issues.append({"kind": "locale_duplicate_key", "lang": lang, "layer": layer,
                           "file": path.name, "line": lineno, "key": key})
            continue
        entries[key] = value
    return entries


def load_all(game_root: Path) -> LocaleSet:
    ls = LocaleSet()
    loc_root = game_root / "TerraInvicta_Data/StreamingAssets/Localization"
    if not loc_root.is_dir():
        return ls
    dlc_root = game_root / "DLC_Content"
    for directory in sorted(p for p in loc_root.iterdir() if p.is_dir()):
        lang = directory.name
        merged = ls.entries.setdefault(lang, {})
        base_files = sorted(p for p in directory.iterdir()
                            if p.is_file() and not p.name.endswith("meta"))
        dlc_files = sorted((p for p in dlc_root.rglob(f"*.{lang}") if p.is_file()),
                           key=lambda p: p.relative_to(dlc_root).as_posix())
        for layer, paths in (("base", base_files), ("dlc", dlc_files)):
            for path in paths:
                ls.sources[path.relative_to(game_root).as_posix()] = sha256_file(path)
                parse_file(path, ls.issues, lang=lang, layer=layer, entries=merged)
    return ls
