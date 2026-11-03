"""Exporting the 2D images that templates reference from Unity asset bundles.

Templates reference resources as `<bundle>/<assetName>` (e.g.
`shipbuildericons/ICO_SteelArmor`). Each bundle ships a plain-text
`<bundle>.manifest` listing its asset paths, which lets us decide which
bundles contain matching *images* without unpacking anything; models, videos
and sounds are never loaded.
"""

from __future__ import annotations

import ast
import gc
import os
import re
import shutil
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable

from .output import sha256_file

IMAGE_EXTENSIONS = {".png", ".jpg", ".jpeg", ".tga", ".psd", ".tif", ".tiff", ".bmp", ".exr"}

# Images the game uses that are hard-coded rather than referenced by templates.
EXTRA_RESOURCES = [
    f"icons_2d/tech_{c}_icon"
    for c in ("energy", "info", "life", "material", "military", "social", "space", "xeno")
]

_SAFE = re.compile(r"[^A-Za-z0-9._ -]+")

# TextMeshPro sprite asset behind `<sprite name="…">` tags in localized text.
INLINE_SPRITE_BUNDLE = "ui"
INLINE_SPRITE_DIR = "sprites"


@dataclass
class Bundle:
    name: str  # lower-case bundle name
    path: Path
    # lower-case image stem -> original stem
    images: dict[str, str] = field(default_factory=dict)


def _manifest_assets(text: str) -> Iterable[str]:
    """Read Unity's Assets list, folding indented plain-scalar continuations."""
    def decode_path(value: str) -> str:
        # Unity quotes paths containing non-ASCII characters with \x/\u escapes.
        if value.startswith('"'):
            return ast.literal_eval(value)
        if value.startswith("'"):
            return value[1:-1].replace("''", "'")
        return value

    in_assets = False
    asset = None
    for line in text.splitlines():
        if not in_assets:
            if line.startswith("Assets:"):
                in_assets = True
            continue
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if line.startswith("- "):
            if asset is not None:
                yield decode_path(asset)
            asset = line[2:].strip()
        elif line[0].isspace():
            if asset is not None:
                asset += " " + line.strip()
        elif ":" in line:  # the next top-level key ends the list
            break
    if asset is not None:
        yield decode_path(asset)


def read_bundle_manifests(bundle_dirs: Iterable[Path]) -> dict[str, Bundle]:
    bundles: dict[str, Bundle] = {}
    for d in bundle_dirs:
        for manifest in sorted(d.glob("*.manifest")):
            path = manifest.with_suffix("")
            if not path.is_file() or path.name == d.name:  # skip the index bundle
                continue
            bundle = Bundle(path.name.lower(), path)
            text = manifest.read_text(encoding="utf-8", errors="replace")
            for asset_path in _manifest_assets(text):
                asset = Path(asset_path)
                if asset.suffix.lower() in IMAGE_EXTENSIONS:
                    bundle.images.setdefault(asset.stem.lower(), asset.stem)
            bundles.setdefault(bundle.name, bundle)
    return bundles


def collect_resources(strings: Iterable[str], bundles: dict[str, Bundle]) -> set[str]:
    """Resource strings (`bundle/name`) whose bundle prefix is a known bundle."""
    found = set(EXTRA_RESOURCES)
    for s in strings:
        if "/" in s and not s.startswith(("event:", "http")):
            if s.split("/", 1)[0].lower() in bundles:
                found.add(s)
    return found


def _file_name(stem: str) -> str:
    return _SAFE.sub("_", stem) + ".png"


@dataclass
class AssetResult:
    resources: dict[str, Any] = field(default_factory=dict)
    bundles: dict[str, Any] = field(default_factory=dict)
    missing: list[str] = field(default_factory=list)


def _plan(resources: set[str], bundles: dict[str, Bundle]):
    """bundle -> {original image stem -> [resource strings]}; plus unresolvable resources."""
    plan: dict[str, dict[str, list[str]]] = defaultdict(lambda: defaultdict(list))
    missing: list[str] = []
    for res in sorted(resources):
        bname, _, asset = res.partition("/")
        bundle = bundles[bname.lower()]
        key = asset.rsplit("/", 1)[-1].lower()
        if key in bundle.images:
            plan[bundle.name][bundle.images[key]].append(res)
        elif matches := [
            s for k, s in bundle.images.items()
            if (k.startswith(key) if key.endswith("_") else re.fullmatch(re.escape(key) + r"_[^\W_]+", k))
        ]:
            # Explicit prefixes and runtime states such as ICO_abductions_on/off.
            for stem in matches:
                plan[bundle.name][stem].append(res)
        else:
            missing.append(res)
    return plan, missing


def _export_bundle(bundle: Bundle, wanted: dict[str, list[str]], out_dir: Path) -> dict[str, dict[str, Any]]:
    import UnityPy  # heavy import, only needed when extracting

    want = {stem.lower(): stem for stem in wanted}
    exported: dict[str, dict[str, Any]] = {}
    env = UnityPy.load(str(bundle.path))
    # Sprites first: they carry atlas cropping; fall back to the raw texture.
    for type_name in ("Sprite", "Texture2D"):
        for obj in env.objects:
            if obj.type.name != type_name:
                continue
            name = obj.peek_name() if hasattr(obj, "peek_name") else None
            if name is None:
                name = obj.read().m_Name
            stem = want.get((name or "").lower())
            if stem is None or stem in exported:
                continue
            data = obj.read()
            try:
                image = data.image
            except Exception as e:  # undecodable texture format
                print(f"  ! {bundle.name}/{name}: {e}", file=sys.stderr)
                continue
            rel = f"{bundle.name}/{_file_name(stem)}"
            dest = out_dir / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            image.save(dest, format="PNG", optimize=False)
            exported[stem] = {"file": rel, "width": image.width, "height": image.height}
    del env
    gc.collect()
    return exported


def _reuse(prev_assets: Path, prev_bundle: dict[str, Any], out_dir: Path, wanted: set[str]) -> dict[str, dict[str, Any]] | None:
    exported = prev_bundle.get("exported")
    if not isinstance(exported, dict):
        return None
    exported = {k: v for k, v in exported.items() if k in wanted}
    for meta in exported.values():
        src, dest = prev_assets / meta["file"], out_dir / meta["file"]
        if not src.is_file():
            return None
        dest.parent.mkdir(parents=True, exist_ok=True)
        if dest.exists():
            dest.unlink()
        try:
            os.link(src, dest)
        except OSError:
            shutil.copy2(src, dest)
    return exported


def extract(
    resources: set[str],
    bundles: dict[str, Bundle],
    out_dir: Path,
    *,
    skip_bundles: set[str] = frozenset(),
    previous: list[tuple[Path, dict[str, Any]]] = (),
) -> AssetResult:
    """Export images to `out_dir`. `previous` = [(assets dir, its index.json)] used to
    reuse output for bundles whose file hash did not change."""
    plan, missing = _plan(resources, bundles)
    result = AssetResult(missing=missing)
    for bname in sorted(plan):
        bundle = bundles[bname]
        if bname in skip_bundles:
            result.bundles[bname] = {"skipped": True}
            continue
        digest = sha256_file(bundle.path)
        exported = None
        for prev_dir, prev_index in previous:
            prev = prev_index.get("bundles", {}).get(bname, {})
            if prev.get("sha256") == digest and set(prev.get("exported", {})) >= set(plan[bname]):
                exported = _reuse(prev_dir, prev, out_dir, set(plan[bname]))
                if exported is not None:
                    exported = {k: v for k, v in exported.items() if k in plan[bname]}
                    print(f"  = {bname}: reused {len(exported)} images")
                    break
        if exported is None:
            print(f"  + {bname}: exporting {len(plan[bname])} images from {bundle.path.name}")
            exported = _export_bundle(bundle, plan[bname], out_dir)
        result.bundles[bname] = {"sha256": digest, "exported": dict(sorted(exported.items()))}
        resource_images: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for stem, res_list in plan[bname].items():
            for res in res_list:
                if stem in exported:
                    resource_images[res].append(exported[stem])
                else:
                    result.missing.append(res)
        for res, images in resource_images.items():
            images.sort(key=lambda image: image["file"])
            entry = dict(images[0])
            if len(images) > 1:
                entry["variants"] = [image["file"] for image in images]
            result.resources[res] = entry
    result.missing = sorted(set(result.missing) - set(result.resources))
    return result


def glyph_box(rect: dict[str, int], sheet_height: int) -> tuple[int, int, int, int]:
    """PIL crop box of a TMP glyph rect (Unity measures y from the bottom)."""
    top = sheet_height - rect["m_Y"] - rect["m_Height"]
    return rect["m_X"], top, rect["m_X"] + rect["m_Width"], top + rect["m_Height"]


def _export_inline_sprites(bundle: Bundle, out_dir: Path) -> dict[str, dict[str, Any]]:
    import UnityPy

    env = UnityPy.load(str(bundle.path))
    by_id = {obj.path_id: obj for obj in env.objects}
    exported: dict[str, dict[str, Any]] = {}
    for obj in env.objects:
        if obj.type.name != "MonoBehaviour":
            continue
        try:
            tree = obj.read_typetree()
        except Exception:
            continue
        if "m_SpriteCharacterTable" not in tree or "m_GlyphTable" not in tree:
            continue
        sheet_ref = by_id.get(tree.get("spriteSheet", {}).get("m_PathID"))
        if sheet_ref is None:
            continue
        sheet = sheet_ref.read().image
        glyphs = {g["m_Index"]: g["m_GlyphRect"] for g in tree["m_GlyphTable"]}
        for char in tree["m_SpriteCharacterTable"]:
            name, rect = char.get("m_Name"), glyphs.get(char.get("m_GlyphIndex"))
            if not name or rect is None or name in exported or rect["m_Width"] <= 0 or rect["m_Height"] <= 0:
                continue
            image = sheet.crop(glyph_box(rect, sheet.height))
            rel = f"{INLINE_SPRITE_DIR}/{_file_name(name)}"
            dest = out_dir / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            image.save(dest, format="PNG", optimize=False)
            exported[name] = {"file": rel, "width": image.width, "height": image.height}
    del env
    gc.collect()
    return exported


def extract_inline_sprites(
    bundles: dict[str, Bundle],
    out_dir: Path,
    *,
    previous: list[tuple[Path, dict[str, Any]]] = (),
) -> dict[str, Any] | None:
    """Export the glyphs of the game's inline sprite sheet (`<sprite name="metal">`) to
    `sprites/<name>.png`. Returns `{"sha256", "exported": {name: image}}`, or None
    without the `ui` bundle."""
    bundle = bundles.get(INLINE_SPRITE_BUNDLE)
    if bundle is None:
        return None
    digest = sha256_file(bundle.path)
    for prev_dir, prev_index in previous:
        prev = prev_index.get("sprites") or {}
        if prev.get("sha256") == digest:
            exported = _reuse(prev_dir, prev, out_dir, set(prev.get("exported", {})))
            if exported is not None:
                print(f"  = inline sprites: reused {len(exported)} images")
                return {"sha256": digest, "exported": dict(sorted(exported.items()))}
    print(f"  + inline sprites: exporting from {bundle.path.name}")
    exported = _export_inline_sprites(bundle, out_dir)
    return {"sha256": digest, "exported": dict(sorted(exported.items()))}
