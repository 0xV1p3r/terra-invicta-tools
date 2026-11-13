import json

from ti_extract.diff import diff_builds


def build(tmp_path, name, records):
    path = tmp_path / name
    templates = path / "templates" / "base"
    templates.mkdir(parents=True)
    (templates / "Test.json").write_text(json.dumps(records))
    (path / "manifest.json").write_text(json.dumps({"game": {"buildId": name}}))
    return path


def test_changed_non_last_variant(tmp_path):
    records = [{"dataName": "Foo", "scenarioTags": ["B", "A"], "value": 1},
               {"dataName": "Foo", "scenarioTags": ["C"], "value": 3}]
    old = build(tmp_path, "old", records)
    records[0]["value"] = 2
    new = build(tmp_path, "new", records)
    assert '- changed `Foo` [tags: A, B]: `value`: 1 → 2' in diff_builds(old, new)


def test_added_removed_variants(tmp_path):
    base = {"dataName": "Foo", "scenarioTags": ["A"]}
    old = build(tmp_path, "old", [base, {"dataName": "Foo", "scenarioTags": ["B"]}])
    new = build(tmp_path, "new", [base, {"dataName": "Foo", "scenarioTags": ["C"]}, {"dataName": "Foo", "scenarioTags": ["D"]}])
    text = diff_builds(old, new)
    assert '- added `Foo` [tags: C]' in text and '- added `Foo` [tags: D]' in text
    assert '- removed `Foo` [tags: B]' in text
    assert '- changed' not in text


def test_tag_edit_of_a_sole_record_is_a_change(tmp_path):
    old = build(tmp_path, "old", [{"dataName": "Foo", "scenarioTags": ["A"]}])
    new = build(tmp_path, "new", [{"dataName": "Foo", "scenarioTags": ["A", "B"]}])
    assert '- changed `Foo`: `scenarioTags`\n' in diff_builds(old, new)


def test_reordered_and_removed_duplicates(tmp_path):
    one, two = ({"dataName": "Foo", "scenarioTags": ["A"], "value": v} for v in (1, 2))
    old = build(tmp_path, "old", [one, two])
    assert diff_builds(old, build(tmp_path, "reordered", [two, one])).endswith("No data changes.\n")
    text = diff_builds(old, build(tmp_path, "new", [two]))
    assert '- removed `Foo` [tags: A]' in text and '- changed' not in text


def test_single_record_output_unchanged(tmp_path):
    old = build(tmp_path, "old", [{"dataName": "Foo", "value": 1}, {"dataName": "Removed"}])
    new = build(tmp_path, "new", [{"dataName": "Foo", "value": 2}, {"dataName": "Added"}])
    assert diff_builds(old, new) == (
        '# Changes: build old → new\n\n## Test (base)\n'
        '- added `Added`\n- removed `Removed`\n- changed `Foo`: `value`: 1 → 2\n\n'
    )


def test_duplicate_tag_identity_preserves_each_occurrence(tmp_path):
    records = [{"dataName": "Foo", "scenarioTags": ["A"], "value": 1}] * 2
    old = build(tmp_path, "old", records)
    new = build(tmp_path, "new", [{**records[0], "value": 2}, records[1]])
    text = diff_builds(old, new)
    assert text.count('- changed `Foo` [tags: A]: `value`: 1 → 2') == 1


def test_tag_order_does_not_change_variant_identity(tmp_path):
    old = build(tmp_path, "old", [{"dataName": "Foo", "scenarioTags": ["B", "A"]}])
    new = build(tmp_path, "new", [{"dataName": "Foo", "scenarioTags": ["A", "B"]}])
    text = diff_builds(old, new)
    assert '- changed `Foo`: `scenarioTags`' in text
    assert '- added' not in text and '- removed' not in text
