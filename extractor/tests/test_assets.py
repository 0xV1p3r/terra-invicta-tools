from pathlib import Path

import pytest

from ti_extract import assets
from ti_extract.assets import Bundle, _plan, read_bundle_manifests


@pytest.mark.parametrize("tail", ["", "Dependencies:\n- not-an-asset.png\n"])
def test_manifest_folds_wrapped_paths_and_reads_until_next_key(tmp_path, tail):
    (tmp_path / "illustrations").touch()
    (tmp_path / "illustrations.manifest").write_text(
        "ManifestFileVersion: 0\n"
        "Assets:\n"
        "- Assets/Art/First.png\n"
        "- Assets/Art/BSBE_Event_Faction_Specific\n"
        "   5.png\n"
        "\n"
        "- Assets/Art/Another\n"
        "  wrapped\n"
        "  name.PNG\n"
        "- Assets/Models/Ship.prefab\n"
        '- "Assets/Art/S\\xFBret\\xE9del\'\\xC9tat.png"\n'
        "- Assets/Art/Last.jpg\n"
        + tail,
        encoding="utf-8",
    )

    bundle = read_bundle_manifests([tmp_path])["illustrations"]

    assert bundle.images == {
        "first": "First",
        "bsbe_event_faction_specific 5": "BSBE_Event_Faction_Specific 5",
        "another wrapped name": "Another wrapped name",
        "sûretédel'état": "Sûretédel'État",
        "last": "Last",
    }


@pytest.mark.parametrize("tail", ["", "Dependencies:\n- ignored.png\n"])
def test_manifest_empty_assets(tmp_path, tail):
    (tmp_path / "empty").touch()
    (tmp_path / "empty.manifest").write_text("Assets: []\n" + tail, encoding="utf-8")

    assert read_bundle_manifests([tmp_path])["empty"].images == {}


def test_plan_suffix_variants_require_one_word_and_preserve_exact_matches():
    stems = ["ICO_abductions_on", "ICO_abductions_off", "ICO_abductions_on_extra",
             "ICO_abductionsOther", "ICO_abductions_bad-state", "ICO_exact", "ICO_exact_on"]
    bundle = Bundle("councilor_missions", Path("unused"), {s.lower(): s for s in stems})
    prefix = "councilor_missions/"
    plan, missing = _plan(
        {prefix + "ICO_ABDUCTIONS", prefix + "ICO_exact", prefix + "ICO_missing"},
        {bundle.name: bundle},
    )

    assert dict(plan[bundle.name]) == {
        "ICO_abductions_on": [prefix + "ICO_ABDUCTIONS"],
        "ICO_abductions_off": [prefix + "ICO_ABDUCTIONS"],
        "ICO_exact": [prefix + "ICO_exact"],
    }
    assert missing == [prefix + "ICO_missing"]


def test_plan_prefix_variants():
    stems = ["Ship_Officer_Admiral_2", "Ship_Officer_Admiral_1", "Ship_Officer_Admiral_3_extra",
             "Ship_Officer_Admiralty_1"]
    bundle = Bundle("icons_2d", Path("unused"), {s.lower(): s for s in stems})
    resource = "icons_2d/Ship_Officer_Admiral_"

    plan, missing = _plan({resource}, {bundle.name: bundle})

    assert dict(plan[bundle.name]) == {s: [resource] for s in stems[:3]}
    assert missing == []


@pytest.mark.parametrize("key,stems", [
    ("ICO_abductions", ["ICO_abductions_on", "ICO_abductions_off"]),
    ("Ship_Officer_Admiral_", ["Ship_Officer_Admiral_2", "Ship_Officer_Admiral_1"]),
    ("Single", ["Single_on"]),
])
def test_extract_variant_index_and_hash_reuse(tmp_path, monkeypatch, key, stems):
    bundle_path = tmp_path / "bundle"
    bundle_path.write_bytes(b"synthetic bundle")
    bundle = Bundle("icons_2d", bundle_path, {s.lower(): s for s in stems})
    resource = f"icons_2d/{key}"
    exported = {
        stem: {"file": f"icons_2d/{stem}.png", "width": i + 10, "height": i + 20}
        for i, stem in enumerate(stems)
    }

    def export_bundle(bundle, wanted, out_dir):
        assert set(wanted) == set(stems)
        for meta in exported.values():
            dest = out_dir / meta["file"]
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(b"synthetic image")
        return exported

    monkeypatch.setattr(assets, "_export_bundle", export_bundle)
    first_dir = tmp_path / "first"
    first = assets.extract({resource}, {bundle.name: bundle}, first_dir)
    sorted_images = sorted(exported.values(), key=lambda image: image["file"])
    expected = dict(sorted_images[0])
    if len(stems) > 1:
        expected["variants"] = [image["file"] for image in sorted_images]
    assert first.resources == {resource: expected}
    assert first.missing == []

    def unexpected_export(*args):
        pytest.fail("unchanged bundle should reuse its exported images")

    monkeypatch.setattr(assets, "_export_bundle", unexpected_export)
    second_dir = tmp_path / "second"
    second = assets.extract(
        {resource}, {bundle.name: bundle}, second_dir,
        previous=[(first_dir, {"bundles": first.bundles})],
    )
    assert second == first
    assert all((second_dir / meta["file"]).read_bytes() == b"synthetic image"
               for meta in exported.values())


@pytest.mark.parametrize("missing_unrequested", [False, True])
def test_reuse_copies_only_requested_stems(tmp_path, monkeypatch, missing_unrequested):
    bundle_path = tmp_path / 'bundle'
    bundle_path.write_bytes(b'bundle')
    bundle = Bundle('icons', bundle_path, {'a': 'A', 'b': 'B'})
    previous = tmp_path / 'previous'
    exported = {}
    for stem in ('A', 'B'):
        path = previous / f'icons/{stem}.png'
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b'image')
        exported[stem] = {'file': f'icons/{stem}.png', 'width': 1, 'height': 1}
    # An unrequested missing file must not force a new export either.
    if missing_unrequested:
        (previous / 'icons/B.png').unlink()
    monkeypatch.setattr(assets, '_export_bundle', lambda *args: pytest.fail('should reuse A'))
    out = tmp_path / 'out'
    result = assets.extract({'icons/A'}, {'icons': bundle}, out, previous=[(previous, {
        'bundles': {'icons': {'sha256': assets.sha256_file(bundle_path), 'exported': exported}}})])
    assert set(result.bundles['icons']['exported']) == {'A'}
    assert [p.name for p in out.rglob('*.png')] == ['A.png']


def test_glyph_box_flips_unity_y():
    # 64×32 glyph at x=10 whose bottom edge is 100 px above the bottom of a 1024 px sheet.
    rect = {"m_X": 10, "m_Y": 100, "m_Width": 64, "m_Height": 32}
    assert assets.glyph_box(rect, 1024) == (10, 892, 74, 924)


def test_inline_sprites_reuse_by_hash(tmp_path, monkeypatch):
    bundle_file = tmp_path / "ui"
    bundle_file.write_bytes(b"bundle")
    prev_dir = tmp_path / "prev"
    (prev_dir / "sprites").mkdir(parents=True)
    (prev_dir / "sprites" / "metal.png").write_bytes(b"png")
    prev_index = {"sprites": {"sha256": assets.sha256_file(bundle_file),
                              "exported": {"metal": {"file": "sprites/metal.png", "width": 1, "height": 1}}}}
    monkeypatch.setattr(assets, "_export_inline_sprites", lambda *a: pytest.fail("should reuse"))
    out = tmp_path / "out"
    result = assets.extract_inline_sprites({"ui": Bundle("ui", bundle_file)}, out, previous=[(prev_dir, prev_index)])
    assert result["exported"]["metal"]["file"] == "sprites/metal.png"
    assert (out / "sprites" / "metal.png").read_bytes() == b"png"
    assert assets.extract_inline_sprites({}, out) is None
