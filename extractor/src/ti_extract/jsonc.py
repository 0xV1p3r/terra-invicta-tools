"""Lenient JSON reader for the game's template files.

The templates are mostly JSON, but some contain `//` and `/* */` comments and
trailing commas. Comments are blanked out (not removed) so that line/column
numbers in parse errors still point at the original file.
Missing commas are inserted only between complete tokens in containers.
Duplicate object keys are reported and reject the whole file, like FullSerializer.
"""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

_TRAILING_COMMA = re.compile(r'("(?:\\.|[^"\\])*")|,(\s*[\]}])')


def strip_comments(text: str) -> str:
    out: list[str] = []
    i, n = 0, len(text)
    in_string = False
    while i < n:
        c = text[i]
        if in_string:
            out.append(c)
            if c == "\\" and i + 1 < n:
                out.append(text[i + 1])
                i += 2
                continue
            if c == '"':
                in_string = False
            i += 1
        elif c == '"':
            in_string = True
            out.append(c)
            i += 1
        elif text.startswith("//", i):
            end = text.find("\n", i)
            end = n if end == -1 else end
            out.append(" " * (end - i))
            i = end
        elif text.startswith("/*", i):
            end = text.find("*/", i + 2)
            end = n if end == -1 else end + 2
            # keep newlines so line numbers stay stable
            out.append("".join(ch if ch == "\n" else " " for ch in text[i:end]))
            i = end
        else:
            out.append(c)
            i += 1
    return "".join(out)


def strip_trailing_commas(text: str) -> str:
    return _TRAILING_COMMA.sub(lambda m: m.group(1) or m.group(2), text)


_TOKEN = re.compile(r'"(?:\\.|[^"\\])*"|[{}\[\],:]|[^\s{}\[\],:"]+')


def insert_missing_commas(text: str) -> str:
    """Track container grammar; never repair colons, invalid tokens or root values.

    Comments must already be blanked. Numeric tokens stay intact, so malformed
    numbers cannot accidentally become several valid array elements.
    """
    stack: list[list[str]] = []
    out = []
    end = 0
    for match in _TOKEN.finditer(text):
        token = match.group()
        starts_value = (
            token[0] in '"[{'
            or token in ('true', 'false', 'null', 'NaN', 'Infinity', '-Infinity')
            or bool(re.fullmatch(r'-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?', token))
        )
        out.append(text[end:match.start()])
        if stack and stack[-1][1] == 'end' and starts_value:
            if stack[-1][0] == '[' or token.startswith('"'):
                out.append(',')
                stack[-1][1] = 'value' if stack[-1][0] == '[' else 'key'
        if token in ('[', '{'):
            if stack:
                stack[-1][1] = 'end'
            stack.append([token, 'value' if token == '[' else 'key'])
        elif token in (']', '}'):
            if stack:
                stack.pop()
        elif stack:
            if token == ',':
                stack[-1][1] = 'value' if stack[-1][0] == '[' else 'key'
            elif token == ':':
                stack[-1][1] = 'value'
            elif stack[-1][1] == 'key' and token.startswith('"'):
                stack[-1][1] = 'colon'
            elif stack[-1][1] == 'value' and starts_value:
                stack[-1][1] = 'end'
        out.append(token)
        end = match.end()
    out.append(text[end:])
    return ''.join(out)


class _ObjectPairs:
    def __init__(self, pairs: list[tuple[str, Any]]):
        self.pairs = pairs


def loads(text: str, issues: list[dict[str, Any]] | None = None, *,
          template: str | None = None, layer: str | None = None) -> Any:
    if text.startswith("\ufeff"):
        text = text[1:]
    parsed = json.loads(strip_trailing_commas(insert_missing_commas(strip_comments(text))), object_pairs_hook=_ObjectPairs)
    context = {k: v for k, v in {"template": template, "layer": layer}.items() if v is not None}

    duplicate = False

    def materialize(value: Any, path: str = "$", data_name: str | None = None) -> Any:
        nonlocal duplicate
        if isinstance(value, _ObjectPairs):
            own_name = dict(value.pairs).get("dataName")
            if isinstance(own_name, str):
                data_name = own_name
            result = {}
            for key, child in value.pairs:
                if key in result:
                    duplicate = True
                    issue = {"kind": "duplicate_json_key", "severity": "error", **context, "key": key, "path": path}
                    if data_name is not None:
                        issue["dataName"] = data_name
                    if issues is not None:
                        issues.append(issue)
                result[key] = materialize(child, f"{path}.{key}", data_name)
            return result
        if isinstance(value, list):
            return [materialize(child, f"{path}[{i}]", data_name) for i, child in enumerate(value)]
        return value

    result = materialize(parsed)
    # fsJsonParser.Dictionary.Add throws before StringSerializationAPI can
    # deserialize any records: the entire source file fails, not just a record.
    if duplicate:
        raise ValueError("duplicate JSON key (whole file rejected by FullSerializer)")
    return result


def load(path: Path, issues: list[dict[str, Any]] | None = None, *,
         template: str | None = None, layer: str | None = None) -> Any:
    return loads(path.read_text(encoding="utf-8-sig"), issues, template=template, layer=layer)
