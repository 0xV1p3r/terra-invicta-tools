"""`ti-extract` command line."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

from . import FORMAT_VERSION, __version__, assets, game, locale, schema, templates
from .diff import diff_builds
from .output import canonical_record, write_json


def _complete_builds(data_dir: Path) -> list[Path]:
    return sorted(p for p in data_dir.iterdir() if p.is_dir() and (p / "manifest.json").is_file()) if data_dir.is_dir() else []


def _write_index(data_dir: Path) -> None:
    builds = []
    for b in _complete_builds(data_dir):
        m = json.loads((b / "manifest.json").read_text(encoding="utf-8"))
        builds.append({
            "dir": b.name,
            "buildId": m["game"]["buildId"],
            "lastUpdated": m["game"].get("lastUpdated"),
            "formatVersion": m["formatVersion"],
        })
    builds.sort(key=lambda b: (b["lastUpdated"] or 0, b["dir"]))
    write_json(data_dir / "index.json", {"latest": builds[-1]["dir"] if builds else None, "builds": builds})


def cmd_info(args: argparse.Namespace) -> int:
    install = game.discover(game.resolve_game_dir(args.game_dir))
    print(f"game dir:  {install.root}")
    print(f"version:   {install.version}")
    print(f"build id:  {install.build_id}  (last updated {install.last_updated})")
    for layer in install.layers:
        n = len(list(layer.templates_dir.glob('*.json')))
        print(f"layer {layer.id:14} {layer.kind:9} {n:3} templates  {layer.dlc or ''}")
    print(f"languages: {', '.join(install.languages)}")
    print(f"bundle dirs: {', '.join(str(d) for d in install.bundle_dirs)}")
    return 0


def cmd_extract(args: argparse.Namespace) -> int:
    started = time.monotonic()
    install = game.discover(game.resolve_game_dir(args.game_dir))
    data_dir = Path(args.data_dir)

    print("reading templates …")
    ts = templates.load_all(install.layers)
    source_hashes = {
        path.relative_to(install.root).as_posix(): digest
        for path, digest in ts.sources.items()
    }
    parse_failed = any(i["kind"] == "parse_error" for i in ts.issues)
    exit_code = int(parse_failed and not args.allow_parse_errors)
    build_id = args.build_id or install.build_id
    if not build_id:
        digest = hashlib.sha256(json.dumps(source_hashes, sort_keys=True).encode()).hexdigest()
        build_id = f"local-{digest[:12]}"
        print(f"no Steam build id found, using {build_id}")
    build_id = str(build_id)

    final = data_dir / build_id
    if (final / "manifest.json").is_file() and not args.force and not parse_failed:
        print(f"build {build_id} already extracted at {final} (use --force to redo)")
        _write_index(data_dir)
        return exit_code

    work = data_dir / f".{build_id}.partial"
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)

    print("writing templates …")
    for layer_id, by_name in ts.files.items():
        for name, tf in by_name.items():
            tf.records = [canonical_record(r, ts.issues, template=name, layer=layer_id) for r in tf.records]
            write_json(work / "templates" / layer_id / f"{name}.json", tf.records)

    print("reading localization …")
    ls = locale.load_all(install.root)
    for lang, entries in ls.entries.items():
        write_json(work / "locale" / lang / "strings.json", dict(sorted(entries.items())))
    source_hashes.update(ls.sources)

    print("discovering schema …")
    sch = schema.discover(ts)
    write_json(work / "schema.json", sch)

    asset_index = None
    if not args.no_assets:
        print("extracting images …")
        bundles = assets.read_bundle_manifests(install.bundle_dirs)
        strings = (v for name in ts.names() for _, rec in ts.records(name)
                   for _, v in schema.walk(rec) if isinstance(v, str))
        resources = assets.collect_resources(strings, bundles)
        previous = []
        for b in reversed(_complete_builds(data_dir)):
            idx = b / "assets" / "index.json"
            if idx.is_file():
                previous.append((b / "assets", json.loads(idx.read_text(encoding="utf-8"))))
        result = assets.extract(resources, bundles, work / "assets",
                                skip_bundles={s.lower() for s in args.skip_bundle}, previous=previous)
        asset_index = {"resources": dict(sorted(result.resources.items())),
                       "bundles": result.bundles, "missing": result.missing}
        if assets.INLINE_SPRITE_BUNDLE not in {s.lower() for s in args.skip_bundle}:
            sprites = assets.extract_inline_sprites(bundles, work / "assets", previous=previous)
            if sprites is not None:
                asset_index["sprites"] = sprites
        write_json(work / "assets" / "index.json", asset_index)

    issues = ts.issues + ls.issues
    write_json(work / "reports" / "issues.json", {
        "issues": issues,
        "danglingRefs": sch["dangling"],
        "missingImages": asset_index["missing"] if asset_index else None,
    })

    manifest = {
        "formatVersion": FORMAT_VERSION,
        "extractor": {"name": "ti-extract", "version": __version__},
        "extractedAt": datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
        "game": {"appId": game.APP_ID, "buildId": build_id, "version": install.version,
                 "lastUpdated": install.last_updated},
        "layers": [
            {
                "id": layer.id,
                "kind": layer.kind,
                "dlc": layer.dlc,
                "folder": layer.folder,
                "templates": sorted(ts.files.get(layer.id, {})),
            }
            for layer in install.layers
        ],
        "languages": sorted(ls.entries),
        "hasAssets": asset_index is not None,
        "counts": {name: sch["templates"][name]["records"] for name in sorted(sch["templates"])},
        "sources": dict(sorted(source_hashes.items())),
    }
    write_json(work / "manifest.json", manifest)  # written last: marks the build complete

    if final.exists():
        shutil.rmtree(final)
    work.rename(final)
    _write_index(data_dir)

    print(f"done: {final}  ({time.monotonic() - started:.0f}s, {len(issues)} issues, "
          f"{len(sch['dangling'])} fields with dangling refs)")
    return exit_code


def cmd_diff(args: argparse.Namespace) -> int:
    text = diff_builds(Path(args.old), Path(args.new), lang=args.lang)
    if args.output:
        Path(args.output).write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)
    return 0


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(prog="ti-extract", description=__doc__)
    p.add_argument("--game-dir", help="Terra Invicta install dir (default: auto-detect via Steam, or $TI_GAME_DIR)")
    sub = p.add_subparsers(dest="cmd", required=True)

    sub.add_parser("info", help="show the detected install and its data layers").set_defaults(fn=cmd_info)

    e = sub.add_parser("extract", help="extract game data into <data-dir>/<build id>/")
    e.add_argument("--data-dir", default="data")
    e.add_argument("--build-id", help="override the Steam build id used as output directory")
    e.add_argument("--force", action="store_true", help="re-extract even if this build exists")
    e.add_argument("--allow-parse-errors", action="store_true", help="exit successfully even if template files fail to parse")
    e.add_argument("--no-assets", action="store_true", help="skip image extraction")
    e.add_argument("--skip-bundle", action="append", default=[], metavar="NAME",
                   help="do not export images from this bundle (repeatable), e.g. illustrations")
    e.set_defaults(fn=cmd_extract)

    d = sub.add_parser("diff", help="changelog between two extracted builds")
    d.add_argument("old")
    d.add_argument("new")
    d.add_argument("--lang", default="en")
    d.add_argument("-o", "--output")
    d.set_defaults(fn=cmd_diff)

    args = p.parse_args(argv)
    return args.fn(args)


if __name__ == "__main__":
    raise SystemExit(main())
