"""Changelog between two extracted builds, keyed by layer/template/dataName."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any


Record = dict[str, Any]


def _tags(record: Record) -> tuple[str, ...]:
    return tuple(sorted(record.get("scenarioTags") or []))


def _load_templates(build: Path) -> dict[tuple[str, str], dict[str, list[Record]]]:
    """Per template file: dataName -> every record with that name (scenario variants), in file order."""
    out: dict[tuple[str, str], dict[str, list[Record]]] = {}
    for f in sorted((build / "templates").glob("*/*.json")):
        records = json.loads(f.read_text(encoding="utf-8"))
        by_name: dict[str, list[Record]] = {}
        for i, record in enumerate(records):
            by_name.setdefault(record.get("dataName", f"#{i}"), []).append(record)
        out[(f.parent.name, f.stem)] = by_name
    return out


def _match(old: list[Record], new: list[Record]) -> tuple[list[Record], list[Record], list[tuple[Record, Record]]]:
    """Pair the variants of one dataName: `(added, removed, changed)`.

    Identical records cancel out first (so reordering is not a change), then records with
    the same tag set pair up; a single leftover on each side is one record whose tags changed.
    """
    old, new = list(old), list(new)
    for record in list(old):
        if record in new:
            old.remove(record)
            new.remove(record)
    changed = []
    for record in list(old):
        other = next((r for r in new if _tags(r) == _tags(record)), None)
        if other is not None:
            old.remove(record)
            new.remove(other)
            changed.append((record, other))
    if len(old) == 1 and len(new) == 1:
        changed.append((old.pop(), new.pop()))
    return new, old, changed


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
        added, removed, changed = [], [], []
        for dn in sorted(ra.keys() | rb.keys()):
            olds, news = ra.get(dn, []), rb.get(dn, [])
            # Variants are named by their tags only where a dataName has several records.
            several = max(len(olds), len(news)) > 1

            def label(record: Record) -> str:
                return f"`{dn}`" + (f" [tags: {', '.join(_tags(record)) or 'none'}]" if several else "")

            more, fewer, pairs = _match(olds, news)
            added += sorted(label(r) for r in more)
            removed += sorted(label(r) for r in fewer)
            for x, y in sorted(pairs, key=lambda pair: _tags(pair[0])):
                changed.append((label(x), x, y, sorted(f for f in x.keys() | y.keys() if x.get(f) != y.get(f))))
        if not (added or removed or changed):
            continue
        lines.append(f"## {template} ({layer})")
        lines += [f"- added {name}" for name in added]
        lines += [f"- removed {name}" for name in removed]
        for name, x, y, fields in changed:
            details = ", ".join(
                f"`{f}`: {json.dumps(x.get(f), ensure_ascii=False)} → {json.dumps(y.get(f), ensure_ascii=False)}"
                if not isinstance(x.get(f), (dict, list)) and not isinstance(y.get(f), (dict, list))
                else f"`{f}`"
                for f in fields
            )
            lines.append(f"- changed {name}: {details}")
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
