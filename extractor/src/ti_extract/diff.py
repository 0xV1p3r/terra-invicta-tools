"""Changelog between two extracted builds, keyed by layer/template/dataName."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


def _load_templates(build: Path) -> dict[tuple[str, str], dict[str, Any]]:
    out: dict[tuple[str, str], dict[str, Any]] = {}
    for f in sorted((build / "templates").glob("*/*.json")):
        records = json.loads(f.read_text(encoding="utf-8"))
        out[(f.parent.name, f.stem)] = {r.get("dataName", f"#{i}"): r for i, r in enumerate(records)}
    return out


def _load_locale(build: Path, lang: str) -> dict[str, str]:
    merged: dict[str, str] = {}
    for f in sorted((build / "locale" / lang).glob("*.json")):
        for k, v in json.loads(f.read_text(encoding="utf-8")).items():
            merged[f"{f.stem}:{k}"] = v
    return merged


def diff_builds(old: Path, new: Path, *, lang: str = "en") -> str:
    a, b = _load_templates(old), _load_templates(new)
    om = json.loads((old / "manifest.json").read_text(encoding="utf-8"))
    nm = json.loads((new / "manifest.json").read_text(encoding="utf-8"))
    lines = [f"# Changes: build {om['game']['buildId']} → {nm['game']['buildId']}", ""]

    for key in sorted(a.keys() | b.keys()):
        layer, template = key
        ra, rb = a.get(key, {}), b.get(key, {})
        added = sorted(rb.keys() - ra.keys())
        removed = sorted(ra.keys() - rb.keys())
        changed = []
        for dn in sorted(ra.keys() & rb.keys()):
            if ra[dn] != rb[dn]:
                fields = sorted(f for f in ra[dn].keys() | rb[dn].keys() if ra[dn].get(f) != rb[dn].get(f))
                changed.append((dn, fields))
        if not (added or removed or changed):
            continue
        lines.append(f"## {template} ({layer})")
        lines += [f"- added `{dn}`" for dn in added]
        lines += [f"- removed `{dn}`" for dn in removed]
        for dn, fields in changed:
            details = ", ".join(
                f"`{f}`: {json.dumps(ra[dn].get(f), ensure_ascii=False)} → {json.dumps(rb[dn].get(f), ensure_ascii=False)}"
                if not isinstance(ra[dn].get(f), (dict, list)) and not isinstance(rb[dn].get(f), (dict, list))
                else f"`{f}`"
                for f in fields
            )
            lines.append(f"- changed `{dn}`: {details}")
        lines.append("")

    la, lb = _load_locale(old, lang), _load_locale(new, lang)
    text_changes = sorted(k for k in la.keys() | lb.keys() if la.get(k) != lb.get(k))
    if text_changes:
        lines.append(f"## Localization ({lang}): {len(text_changes)} keys added/removed/changed")
        lines += [f"- `{k}`" for k in text_changes[:500]]
        if len(text_changes) > 500:
            lines.append(f"- … {len(text_changes) - 500} more")
        lines.append("")
    if len(lines) == 2:
        lines.append("No data changes.")
    return "\n".join(lines) + "\n"
