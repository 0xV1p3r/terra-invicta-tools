"""Reading template files of every layer into plain records."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from . import jsonc
from .game import Layer
from .output import sha256_file


@dataclass
class TemplateFile:
    layer: str
    name: str  # e.g. "TITechTemplate"
    source: Path
    sha256: str
    records: list[dict[str, Any]]


@dataclass
class TemplateSet:
    # layer id -> template name -> file
    files: dict[str, dict[str, TemplateFile]] = field(default_factory=dict)
    issues: list[dict[str, Any]] = field(default_factory=list)
    sources: dict[Path, str] = field(default_factory=dict)

    def names(self) -> list[str]:
        return sorted({name for by_name in self.files.values() for name in by_name})

    def records(self, template: str):
        """Yield (layer, record) for one template over all layers."""
        for layer, by_name in self.files.items():
            if template in by_name:
                for rec in by_name[template].records:
                    yield layer, rec


def _issue(issues: list, kind: str, **details: Any) -> None:
    issues.append({"kind": kind, **details})


def load_layer(layer: Layer, out: TemplateSet) -> None:
    by_name: dict[str, TemplateFile] = {}
    # DLC directories are already individually discovered, including nested ones.
    paths = layer.templates_dir.rglob("*.json") if layer.kind == "base" else layer.templates_dir.glob("*.json")
    for path in sorted(paths, key=lambda p: p.relative_to(layer.templates_dir).as_posix()):
        out.sources[path] = sha256_file(path)
        name = path.stem
        try:
            data = jsonc.load(path, out.issues, template=name, layer=layer.id)
        except ValueError as e:
            _issue(out.issues, "parse_error", layer=layer.id, template=name, error=str(e))
            continue
        if not isinstance(data, list):
            data = [data]
        records: list[dict[str, Any]] = []
        seen: set[str] = set()
        for i, rec in enumerate(data):
            if not isinstance(rec, dict):
                _issue(out.issues, "non_object_record", layer=layer.id, template=name, index=i)
                continue
            dn = rec.get("dataName")
            if not isinstance(dn, str) or not dn:
                _issue(out.issues, "missing_data_name", layer=layer.id, template=name, index=i)
            elif dn in seen:
                _issue(out.issues, "duplicate_data_name", layer=layer.id, template=name, dataName=dn)
            else:
                seen.add(dn)
            records.append(rec)
        if name in by_name:
            by_name[name].records.extend(records)
        else:
            by_name[name] = TemplateFile(layer.id, name, path, out.sources[path], records)
    out.files[layer.id] = by_name


def load_all(layers: list[Layer]) -> TemplateSet:
    ts = TemplateSet()
    for layer in layers:
        load_layer(layer, ts)
    for layer in layers[1:]:
        for name in ts.files.get(layer.id, {}):
            if name not in ts.files["base"]:
                _issue(ts.issues, "dlc_only_template", layer=layer.id, template=name)
    return ts
