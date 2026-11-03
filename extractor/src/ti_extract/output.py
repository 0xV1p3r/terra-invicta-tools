"""Deterministic file output so that re-extracting an unchanged game yields identical bytes."""

from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
from typing import Any


def canonical_record(value: Any, issues: list | None = None, *,
                     template: str | None = None, layer: str | None = None,
                     data_name: str | None = None, path: str = "$") -> Any:
    """Sort keys with `dataName` first, replacing and reporting non-finite numbers."""
    if isinstance(value, float) and not math.isfinite(value):
        if issues is not None:
            issues.append({"kind": "non_finite_number", "template": template,
                           "layer": layer, "dataName": data_name, "path": path})
        return None
    context = {"template": template, "layer": layer, "data_name": data_name}
    if isinstance(value, dict):
        context["data_name"] = value.get("dataName", data_name)
        keys = sorted(value, key=lambda k: (k != "dataName", k))
        return {k: canonical_record(value[k], issues, **context, path=f"{path}.{k}") for k in keys}
    if isinstance(value, list):
        return [canonical_record(v, issues, **context, path=f"{path}[{i}]") for i, v in enumerate(value)]
    return value


def dumps(value: Any, *, compact: bool = False) -> str:
    if compact:
        return json.dumps(value, allow_nan=False, ensure_ascii=False, separators=(",", ":")) + "\n"
    return json.dumps(value, allow_nan=False, ensure_ascii=False, indent=1) + "\n"


def write_json(path: Path, value: Any, *, compact: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(dumps(value, compact=compact), encoding="utf-8", newline="\n")


def sha256_file(path: Path, chunk: int = 1 << 20) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while block := f.read(chunk):
            h.update(block)
    return h.hexdigest()
