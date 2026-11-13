#!/usr/bin/env python3
"""Read-only corpus audit for save-files.md; this is not the proposed parser.

Usage: python3 docs/research/inspect_saves.py /path/to/Saves > inventory.json
Uses Python's nonstandard JSON constant support to audit FullSerializer output.
"""

import argparse
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path


PREFIX = "PavonisInteractive.TerraInvicta."


def rows(value):
    if value == {}:
        return []
    if not isinstance(value, list):
        raise ValueError("Expected a Key/Value array or empty dictionary")
    return value


def audit(path):
    compressed = path.read_bytes()
    raw = gzip.decompress(compressed)
    constants = Counter()

    def constant(token):
        constants[token] += 1
        return float(token)

    root = json.loads(raw, parse_constant=constant)
    states = root["gamestates"]
    by_type = {name: rows(value) for name, value in states.items()}
    ids = set()
    for records in by_type.values():
        for row in records:
            key = row["Key"]["value"]
            if key != row["Value"]["ID"]["value"] or key in ids:
                raise ValueError(f"{path.name}: mismatched or duplicate entity ID {key}")
            ids.add(key)

    def singleton(name):
        records = by_type[PREFIX + name]
        if len(records) != 1:
            raise ValueError(f"{path.name}: expected one {name}")
        return records[0]["Value"]

    serializer_ids = set()
    serializer_refs = set()
    special_keys = Counter()
    stack = [root]
    while stack:
        value = stack.pop()
        if isinstance(value, dict):
            for key in ("$id", "$ref", "$type"):
                if key in value:
                    special_keys[key] += 1
            if "$id" in value:
                serializer_ids.add(value["$id"])
            if "$ref" in value:
                serializer_refs.add(value["$ref"])
            stack.extend(value.values())
        elif isinstance(value, list):
            stack.extend(value)
    if serializer_refs - serializer_ids:
        raise ValueError(f"{path.name}: unresolved FullSerializer references")

    time = singleton("TITimeState")
    global_values = singleton("TIGlobalValuesState")
    research = singleton("TIGlobalResearchState")
    humans = [row["Value"] for row in by_type[PREFIX + "TIPlayerState"]
              if row["Value"]["isAI"] is False]
    if len(humans) != 1:
        raise ValueError(f"{path.name}: expected one human player in this corpus")
    factions = {row["Key"]["value"]: row["Value"]
                for row in by_type[PREFIX + "TIFactionState"]}
    faction = factions[humans[0]["faction"]["value"]]
    progress = faction["currentProjectProgress"]
    weights = faction["researchWeights"]

    def paused(project):
        slot = project["slot"]
        return (slot >= 6
                or (slot == 4 and not faction["orgProjectSlotUnlocked"])
                or (slot == 5 and not faction["habProjectSlotUnlocked"]))

    result = {
        "file": path.name,
        "compressedBytes": len(compressed),
        "decompressedBytes": len(raw),
        "decompressedSha256": hashlib.sha256(raw).hexdigest(),
        "utf8Bom": raw.startswith(b"\xef\xbb\xbf"),
        "rootKeys": list(root),
        "typeBuckets": len(by_type),
        "entityRows": len(ids),
        "emptyObjectBuckets": [name for name, value in states.items() if value == {}],
        "nonfiniteConstants": dict(constants),
        "serializerKeys": dict(special_keys),
        "entityIdsUniqueAndMatchKeys": True,
        "serializerReferencesResolve": True,
        "campaignStartVersion": global_values.get("campaignStartVersion"),
        "latestSaveVersion": global_values.get("latestSaveVersion"),
        "realWorldCampaignStart": global_values.get("realWorldCampaignStart"),
        "scenarioMetaTemplateName": time.get("scenarioMetaTemplateName"),
        "startTimeTemplateName": time.get("templateName"),
        "currentDateTime": time["currentDateTime"],
        "humanPlayers": len(humans),
        "humanFactionId": humans[0]["faction"]["value"],
        "humanFactionTemplateName": faction["templateName"],
        "finishedTechCount": len(research["finishedTechsNames"]),
        "finishedProjectEntryCount": len(faction["finishedProjectNames"]),
        "finishedProjectUniqueCount": len(set(faction["finishedProjectNames"])),
        "availableProjectCount": len(faction["availableProjectNames"]),
        "activeProjectTriggerCount": len(faction["activeProjectTriggers"]),
        "missedProjectCount": len(faction["missedProjects"]),
        "researchSpeedMultiplier": global_values.get("scenarioCustomizations", {}).get("researchSpeedMultiplier"),
        "globalSlots": [
            {"slot": i, "techTemplateName": item["techTemplateName"],
             "accumulatedResearch": item["accumulatedResearch"]}
            for i, item in enumerate(research["techProgress"])
        ],
        "projectProgress": [
            {**item, "paused": paused(item),
             "weight": weights[item["slot"]] if 0 <= item["slot"] < len(weights) else None}
            for item in progress
        ],
    }
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    paths = sorted(args.directory.glob("*.gz"))
    if not paths:
        parser.error("No .gz saves found")
    results = [audit(path) for path in paths]
    print(json.dumps({"auditFormatVersion": 1, "saves": results}, indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
