import json
import math
from pathlib import Path

import pytest

from ti_extract import cli, game, jsonc, locale, templates
from ti_extract.game import Layer
from ti_extract.locale import parse_line
from ti_extract.output import canonical_record, sha256_file


def test_jsonc_comments_and_trailing_commas():
    text = '''﻿[
      {"dataName": "a", // line comment
       "url": "http://x//y", /* block
       comment */ "list": [1, 2, ],
       "s": "has \\" quote, ]",},
    ]'''
    assert jsonc.loads(text) == [{"dataName": "a", "url": "http://x//y", "list": [1, 2], "s": 'has " quote, ]'}]


def test_jsonc_error_positions_are_preserved():
    with pytest.raises(ValueError) as e:
        jsonc.loads('/* a\nb */\n{"x": }')
    assert "line 3" in str(e.value)


def test_jsonc_duplicate_keys_reject_file_with_record_context(tmp_path):
    path = tmp_path / "TITestTemplate.json"
    path.write_text('''[
      {"value": 1, "value": 2, "nested": {"x": 3, "x": 4}, "dataName": "a"},
      {"dataName": "a", "value": 5}
    ]''')
    ts = templates.load_all([Layer("base", "base", tmp_path)])
    assert "TITestTemplate" not in ts.files["base"]
    assert any(i["kind"] == "parse_error" for i in ts.issues)
    duplicates = [i for i in ts.issues if i["kind"] == "duplicate_json_key"]
    assert duplicates == [
        {"kind": "duplicate_json_key", "severity": "error", "template": "TITestTemplate", "layer": "base",
         "key": "value", "path": "$[0]", "dataName": "a"},
        {"kind": "duplicate_json_key", "severity": "error", "template": "TITestTemplate", "layer": "base",
         "key": "x", "path": "$[0].nested", "dataName": "a"},
    ]


def test_jsonc_reports_duplicates_even_in_replaced_objects():
    issues = []
    with pytest.raises(ValueError, match="duplicate JSON key"):
        jsonc.loads('{"a": {"x": 1, "x": 2}, "a": 3}', issues)
    assert [i["key"] for i in issues] == ["x", "a"]
    assert all("dataName" not in i for i in issues)


def test_jsonc_nonfinite_numbers_and_case_sensitive_keys():
    data = jsonc.loads('{"n": NaN, "i": Infinity, "I": -Infinity}')
    assert math.isnan(data["n"])
    assert data["i"] == math.inf
    assert data["I"] == -math.inf


@pytest.mark.parametrize("text, expected", [
    ('[1 2]', [1, 2]),
    ('{"x": 1 "y": 2}', {"x": 1, "y": 2}),
    ('[{}{}[]"a" true false null]', [{}, {}, [], "a", True, False, None]),
    ('{"a": [1 /* comment */ 2] "b": {"x": "a,b" "y": "//"}}',
     {"a": [1, 2], "b": {"x": "a,b", "y": "//"}}),
    ('["a\\\"b" // comment\n "c"]', ['a"b', 'c']),
])
def test_jsonc_missing_commas(text, expected):
    assert jsonc.loads(text) == expected


@pytest.mark.parametrize("text", ['[1e 2]', '[1.2.3]', '{"x" 1}', '[1,,2]', '1 2', '{"x": }'])
def test_jsonc_missing_comma_repair_does_not_hide_other_errors(text):
    with pytest.raises(ValueError):
        jsonc.loads(text)


@pytest.mark.parametrize("line,expected", [
    ("A.displayName.X=Hello", ("A.displayName.X", "Hello")),
    ("A.description.X=see http://a.b/c\t\t\t// translator note", ("A.description.X", "see http:")),
    ("A.b.X=1 = 2\r\n", ("A.b.X", "1 = 2")),
    ("// full comment", None),
    ("   ", None),
    ("", None),
    ("A.displayName.NoValue", None),
    ("=NoKey", None),
    (" Key =  leading and trailing   ", (" Key ", "  leading and trailing")),
    ("A=one\ttwo\t // comment", ("A", "onetwo")),
    ("A=one//comment//more", ("A", "one")),
    ("A=<br/>next<br/>", ("A", "\nnext\n")),
    ("A=<h>title</h>", ("A", "<color=#EC9933>title</color>")),
    ("A=<rcol>right</rcol>", ("A", '<line-height=0.01%>\n<align="right">right</align></line-height>')),
    ("A=<sp>hello<sp>   ", ("A", " hello ")),
    ("A=value\v\u00a0   ", ("A", "value\v\u00a0")),
    ("A=", None),
    ("A=\t  //comment", None),
    ("A=<skip/>", ("A", "")),
    ("A= <skip/>   ", ("A", " ")),
    ("A=a<skip/>b", ("A", "ab")),
    ('A=literal \\n \\" {0} <i>text</i>', ("A", 'literal \\n \\" {0} <i>text</i>')),
])
def test_locale_line(line, expected):
    assert parse_line(line) == expected


def _write(root, relative, text):
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path


def test_locale_file_line_endings_and_other_whitespace(tmp_path):
    path = tmp_path / "test.en"
    path.write_bytes("\ufeffa=first\r\nb=second\rc=value\v\u2028\n".encode("utf-8"))
    assert locale.parse_file(path, [], lang="en", layer="base") == {
        "a": "first", "b": "second", "c": "value\v\u2028"}


def test_locale_merge_base_first_wins_then_all_dlc_override(tmp_path):
    loc = "TerraInvicta_Data/StreamingAssets/Localization/en"
    # Create in reverse lexical order to exercise sorting.
    b = _write(tmp_path, f"{loc}/b.en", "shared=second\nbase=second\nsame=same\n")
    a = _write(tmp_path, f"{loc}/a.anything", "\ufeffshared=first\nbase=first\nsame=same\nwithin=first\nwithin=last\nhidden=<skip/>\nhidden=shown\n")
    _write(tmp_path, f"{loc}/ignoredmeta", "ignored=meta\n")
    _write(tmp_path, f"{loc}/sub/nested.en", "ignored=nested\n")
    z = _write(tmp_path, "DLC_Content/Z/odd/folder/en/z.en", "shared=dlc-z\nshared=last\nhidden=\n")
    d = _write(tmp_path, "DLC_Content/A/outside-localization/a.en", "shared=dlc-a\nnew=dlc\n")
    _write(tmp_path, "Mods/Enabled/mod/a.en", "shared=mod\n")
    _write(tmp_path, "TerraInvicta_Data/StreamingAssets/Localization/deu/a.deu", "base=Deutsch\n")
    ls = locale.load_all(tmp_path)
    assert ls.entries == {
        "en": {"shared": "last", "base": "first", "same": "same", "within": "first", "hidden": "", "new": "dlc"},
        "deu": {"base": "Deutsch"},
    }
    assert len(ls.issues) == 5
    assert {i["kind"] for i in ls.issues} == {"locale_duplicate_key"}
    assert {i["key"] for i in ls.issues} == {"shared", "base", "same", "within", "hidden"}
    expected = [a, b, z, d, tmp_path / "TerraInvicta_Data/StreamingAssets/Localization/deu/a.deu"]
    assert ls.sources == {p.relative_to(tmp_path).as_posix(): sha256_file(p) for p in expected}


@pytest.mark.parametrize("folder,expected", [
    ("DarkSkies/2003_Scenario/Templates", "dlc.darkskies.2003_scenario.templates"),
    ("DarkSkies/Broken Earth Scenario/Templates", "dlc.darkskies.broken_earth_scenario.templates"),
    ("Other DLC/Nested/Extras", "dlc.other_dlc.nested.extras"),
])
def test_layer_id(folder, expected):
    assert game.layer_id(Path(folder)) == expected


def test_discover_recursive_directory_layers_in_load_order(tmp_path):
    _write(tmp_path, "TerraInvicta_Data/StreamingAssets/Templates/TIBase.json", "[]")
    for folder in ("Z/Extras", "A/Scenario/Templates", "A/Scenario/Templates/Nested"):
        _write(tmp_path, f"DLC_Content/{folder}/TITest.json", "[]")
    _write(tmp_path, "DLC_Content/A/Scenario/Templates/Another.json", "[]")
    install = game.discover(tmp_path)
    assert [(l.id, l.kind, l.dlc, l.folder) for l in install.layers] == [
        ("base", "base", None, None),
        ("dlc.a.scenario.templates", "dlc", "A", "A/Scenario/Templates"),
        ("dlc.a.scenario.templates.nested", "dlc", "A", "A/Scenario/Templates/Nested"),
        ("dlc.z.extras", "dlc", "Z", "Z/Extras"),
    ]
    ts = templates.load_all(install.layers)
    assert set(ts.files["dlc.a.scenario.templates"]) == {"TITest", "Another"}
    assert install.version is None


def _prefixed(value):
    return len(value).to_bytes(4, "little", signed=True) + value


@pytest.mark.parametrize("version", [b"1.0.53b", b"1.2.3", b"1.0.53b12"])
def test_extract_version(version):
    blob = b"noise1.2.3\x00" + _prefixed(b"2020.3.49f1") + b"\x00" + _prefixed(version) + b"\x00"
    assert game.extract_version(blob) == version.decode("ascii")


@pytest.mark.parametrize("blob", [b"", b"1.0.53b", b"\x06\x00\x00\x001.0.53b", _prefixed(b"2020.3.49f1")])
def test_extract_version_missing_or_invalid(blob):
    assert game.extract_version(blob) is None


def test_extract_manifest_and_locale_layout(tmp_path):
    root = tmp_path / "game"
    _write(root, "TerraInvicta_Data/StreamingAssets/Templates/TITest.json", '[{"dataName": "x", "disable": true, "scenarioTags": ["tag"]}]')
    dlc = _write(root, "DLC_Content/DarkSkies/Other/Templates/TITest.json", '[{"dataName": "x", "extra": 2}]')
    loc = _write(root, "TerraInvicta_Data/StreamingAssets/Localization/en/a.en", "z=<skip/>\na=base\n")
    dlc_loc = _write(root, "DLC_Content/DarkSkies/elsewhere/a.en", "a=override\n")
    (root / "TerraInvicta_Data/globalgamemanagers").write_bytes(_prefixed(b"1.0.53b"))
    output = tmp_path / "output"
    assert cli.main(["--game-dir", str(root), "extract", "--data-dir", str(output),
                     "--build-id", "test", "--no-assets"]) == 0
    build = output / "test"
    manifest = json.loads((build / "manifest.json").read_text())
    assert manifest["formatVersion"] == 1
    assert manifest["game"]["version"] == "1.0.53b"
    assert manifest["languages"] == ["en"]
    assert manifest["layers"] == [
        {"id": "base", "kind": "base", "dlc": None, "folder": None, "templates": ["TITest"]},
        {"id": "dlc.darkskies.other.templates", "kind": "dlc", "dlc": "DarkSkies",
         "folder": "DarkSkies/Other/Templates", "templates": ["TITest"]},
    ]
    for path in [loc, dlc_loc, dlc]:
        assert manifest["sources"][path.relative_to(root).as_posix()] == sha256_file(path)
    assert [p.name for p in (build / "locale/en").glob("*.json")] == ["strings.json"]
    strings = json.loads((build / "locale/en/strings.json").read_text())
    assert list(strings) == ["a", "z"]
    assert strings == {"a": "override", "z": ""}
    assert json.loads((build / "templates/base/TITest.json").read_text()) == [
        {"dataName": "x", "disable": True, "scenarioTags": ["tag"]}]
    assert json.loads((build / "templates/dlc.darkskies.other.templates/TITest.json").read_text()) == [
        {"dataName": "x", "extra": 2}]


def test_canonical_record_order():
    rec = canonical_record({"z": 1, "dataName": "n", "a": {"y": 1, "b": 2}})
    assert list(rec) == ["dataName", "a", "z"]
    assert list(rec["a"]) == ["b", "y"]


@pytest.mark.parametrize("allow", [False, True])
def test_parse_errors_exit_after_reports(tmp_path, allow):
    root = tmp_path / "game"
    _write(root, "TerraInvicta_Data/StreamingAssets/Templates/Bad.json", '[{"dataName": ]')
    output = tmp_path / "out"
    args = ["--game-dir", str(root), "extract", "--data-dir", str(output),
            "--build-id", "test", "--no-assets"]
    if allow:
        args.append("--allow-parse-errors")
    assert cli.main(args) == (0 if allow else 1)
    report = json.loads((output / "test/reports/issues.json").read_text())
    assert report["issues"][0]["kind"] == "parse_error"


def test_nonfinite_output_and_report(tmp_path):
    from ti_extract.output import dumps
    root = tmp_path / "game"
    _write(root, "TerraInvicta_Data/StreamingAssets/Templates/Test.json",
           '[{"dataName":"x", "nested":[NaN, Infinity, -Infinity, 1e999]}]')
    output = tmp_path / "out"
    assert cli.main(["--game-dir", str(root), "extract", "--data-dir", str(output),
                     "--build-id", "test", "--no-assets"]) == 0
    build = output / "test"
    assert json.loads((build / "templates/base/Test.json").read_text())[0]["nested"] == [None] * 4
    issues = json.loads((build / "reports/issues.json").read_text())["issues"]
    assert issues == [{"kind": "non_finite_number", "template": "Test", "layer": "base",
                       "dataName": "x", "path": f"$.nested[{i}]"} for i in range(4)]
    for compact in (False, True):
        with pytest.raises(ValueError):
            dumps(float('nan'), compact=compact)


def test_recursive_base_repeated_basenames_and_nested_dlc(tmp_path):
    prefix = "TerraInvicta_Data/StreamingAssets/Templates"
    paths = [_write(tmp_path, f"{prefix}/{folder}Test.json", f'[{{"dataName":"{name}"}}]')
             for folder, name in [("z/", "last"), ("", "first"), ("a/", "middle")]]
    _write(tmp_path, "DLC_Content/A/Templates/Test.json", '[{"dataName":"dlc"}]')
    _write(tmp_path, "DLC_Content/A/Templates/Nested/Test.json", '[{"dataName":"nested"}]')
    ts = templates.load_all(game.discover(tmp_path).layers)
    assert [r['dataName'] for r in ts.files['base']['Test'].records] == ['first', 'middle', 'last']
    assert [r['dataName'] for layer, r in ts.records('Test')] == ['first', 'middle', 'last', 'dlc', 'nested']
    assert len(ts.sources) == 5
    assert all(ts.sources[p] == sha256_file(p) for p in paths)


def test_schema_independent_of_input_order(tmp_path):
    from ti_extract import schema
    from ti_extract.output import dumps
    def discover(reverse):
        ts = templates.TemplateSet()
        for layer in (['z', 'base'] if reverse else ['base', 'z']):
            ts.files[layer] = {}
            for name in (['Z', 'A'] if reverse else ['A', 'Z']):
                records = [{"dataName": "shared", "ref": "shared", "value": v} for v in range(5)]
                if reverse:
                    records.reverse()
                ts.files[layer][name] = templates.TemplateFile(layer, name, tmp_path, '', records)
        return schema.discover(ts)
    assert dumps(discover(False)) == dumps(discover(True))
    assert all(r['targets'] == ['A', 'Z'] for r in discover(False)['refFields'])
