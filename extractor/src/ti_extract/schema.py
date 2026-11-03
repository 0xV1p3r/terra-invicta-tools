"""Data-driven schema discovery.

For every template, records each field path (`a`, `a[]`, `a[].b`), the JSON
types seen and, for string values, which templates own a record with that
`dataName`. Consumers use this to build a generic reference graph without
hard-coding every field, and to notice schema drift between game builds.
"""

from __future__ import annotations

import json

from collections import Counter, defaultdict
from typing import Any, Iterator

from .templates import TemplateSet

# A field counts as a reference when at least this share of its string values resolve.
REF_THRESHOLD = 0.5
MAX_EXAMPLES = 3
# Fields that hold display text or the record's own key, never references.
NON_REF_FIELDS = {"dataName", "friendlyName", "displayName", "notes", "Notes"}


def _json_type(v: Any) -> str:
    if v is None:
        return "null"
    if isinstance(v, bool):
        return "bool"
    if isinstance(v, int):
        return "int"
    if isinstance(v, float):
        return "float"
    if isinstance(v, str):
        return "string"
    if isinstance(v, list):
        return "array"
    return "object"


def walk(value: Any, path: str = "") -> Iterator[tuple[str, Any]]:
    yield path, value
    if isinstance(value, dict):
        for k, v in value.items():
            yield from walk(v, f"{path}.{k}" if path else k)
    elif isinstance(value, list):
        for v in value:
            yield from walk(v, f"{path}[]")


def data_name_index(ts: TemplateSet) -> dict[str, set[str]]:
    index: dict[str, set[str]] = defaultdict(set)
    for name in ts.names():
        for _, rec in ts.records(name):
            dn = rec.get("dataName")
            if isinstance(dn, str):
                index[dn].add(name)
    return index


def discover(ts: TemplateSet) -> dict[str, Any]:
    index = data_name_index(ts)
    templates: dict[str, Any] = {}
    ref_fields: list[dict[str, Any]] = []
    dangling: list[dict[str, Any]] = []

    for name in ts.names():
        counts = {layer: len(by_name[name].records) for layer, by_name in ts.files.items() if name in by_name}
        types: dict[str, Counter] = defaultdict(Counter)
        present: Counter = Counter()
        strings: Counter = Counter()
        refs: dict[str, Counter] = defaultdict(Counter)
        unresolved: dict[str, Counter] = defaultdict(Counter)
        examples: dict[str, list] = defaultdict(list)

        for _, rec in ts.records(name):
            seen_paths: set[str] = set()
            for path, v in walk(rec):
                if not path:
                    continue
                types[path][_json_type(v)] += 1
                seen_paths.add(path)
                if not isinstance(v, (dict, list)) and v not in examples[path]:
                    examples[path].append(v)
                leaf = path.rsplit(".", 1)[-1].removesuffix("[]")
                if isinstance(v, str) and v and leaf not in NON_REF_FIELDS:
                    strings[path] += 1
                    targets = index.get(v)
                    if targets:
                        for t in targets:
                            refs[path][t] += 1
                    else:
                        unresolved[path][v] += 1
            present.update(seen_paths)

        fields: dict[str, Any] = {}
        for path in sorted(types):
            entry: dict[str, Any] = {
                "present": present[path],
                "types": dict(sorted(types[path].items())),
                "examples": sorted(examples[path], key=lambda v: (type(v).__name__, json.dumps(v, ensure_ascii=False)))[:MAX_EXAMPLES],
            }
            if refs[path]:
                entry["refs"] = dict(sorted(refs[path].items()))
                resolved = strings[path] - sum(unresolved[path].values())
                ratio = resolved / strings[path]
                entry["refRatio"] = round(ratio, 4)
                if ratio >= REF_THRESHOLD:
                    targets = sorted(refs[path], key=lambda t: (-refs[path][t], t))
                    ref_fields.append({"template": name, "path": path, "targets": targets, "ratio": entry["refRatio"]})
                    if unresolved[path]:
                        dangling.append({"template": name, "path": path,
                                         "values": dict(sorted(unresolved[path].items()))})
            fields[path] = entry
        templates[name] = {"records": dict(sorted(counts.items())), "fields": fields}

    ambiguous = {dn: sorted(t) for dn, t in sorted(index.items()) if len(t) > 1}
    return {
        "templates": templates,
        "refFields": ref_fields,
        "ambiguousDataNames": ambiguous,
        "dangling": dangling,
    }
