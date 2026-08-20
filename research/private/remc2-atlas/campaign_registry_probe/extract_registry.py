#!/usr/bin/env python3
"""Extract the pinned remc2 ``str_D4C48ar`` class/action registry.

The extractor reads the two source files from the nested Git object at the
pinned commit.  It never reads the mutable remc2 worktree and never emits
code.  The output is a deterministic JSON measurement of the class rows,
action/constructor rows, stored callback addresses, and best-effort callback
identities from the two compatibility dispatch switches.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, NoReturn

PINNED_COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
TABLE_SOURCE_PATH = "remc2/engine/EventsFunctions.cpp"
DISPATCH_SOURCE_PATH = "remc2/engine/Events.cpp"
SCHEMA = "mc2-str-d4c48ar-registry/v1"

CLASS_TABLE_RE = re.compile(
    r"std::array\s*<\s*type_D4C52ar\s*,\s*17\s*>\s*"
    r"str_D4C48ar\s*=\s*\{(?P<body>.*?)\};",
    re.DOTALL,
)
ARRAY_RE = re.compile(
    r"type_D4C52ar2\s+"
    r"(?P<name>x_DWORD_D4C52ar_str(?P<class>[0-9A-Fa-f])(?P<kind>[01]))"
    r"\[(?P<size>\d+)\]\s*=\s*\{(?P<body>.*?)\};",
    re.DOTALL,
)
NUMBER_RE = re.compile(r"0x[0-9A-Fa-f]+|\b\d+\b")
CLASS_VALUE_RE = re.compile(
    r"0x[0-9A-Fa-f]+|\b\d+\b|\bNULL\b|\bx_DWORD_D4C52ar_str[0-9A-Fa-f]+\b"
)
CASE_RE = re.compile(r"case\s+0x(?P<address>[0-9A-Fa-f]+)\s*:")
CALL_RE = re.compile(r"\b(?P<name>[A-Za-z_]\w*)\s*\(")

CONTROL_CALLS = {
    "catch",
    "const_cast",
    "do",
    "for",
    "if",
    "reinterpret_cast",
    "return",
    "sizeof",
    "static_cast",
    "switch",
    "while",
}


def fail(message: str) -> NoReturn:
    raise ValueError(message)


def strip_comments_preserving_lines(text: str) -> str:
    def replace(match: re.Match[str]) -> str:
        return "".join("\n" if char == "\n" else " " for char in match.group(0))

    return re.sub(r"//[^\n]*|/\*.*?\*/", replace, text, flags=re.DOTALL)


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def repo_root_from_here() -> Path:
    here = Path(__file__).resolve().parent
    for candidate in (here, *here.parents):
        if (candidate / "AGENTS.md").is_file() and (candidate / "upstream").is_dir():
            return candidate
    fail("cannot locate MagicCarpet repository root from extractor path")


def read_pinned_source(repo_root: Path, path: str) -> tuple[str, str]:
    upstream = repo_root / "upstream/magic-carpet-2-hd"
    try:
        result = subprocess.run(
            ["git", "-C", str(upstream), "show", f"{PINNED_COMMIT}:{path}"],
            check=True,
            capture_output=True,
            timeout=30,
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        fail(f"cannot read pinned source {path} at {PINNED_COMMIT}: {exc}")
    try:
        text = result.stdout.decode("utf-8")
    except UnicodeDecodeError as exc:
        fail(f"pinned source {path} is not UTF-8: {exc}")
    return text, hashlib.sha256(result.stdout).hexdigest()


def parse_number(token: str) -> int:
    return int(token, 0)


def hex32(value: int) -> str:
    return f"0x{value:08X}"


def parse_dispatcher_cases(
    source: str, function_name: str, next_function_name: str, role: str
) -> dict[int, dict[str, Any]]:
    start = source.find(function_name)
    if start < 0:
        fail(f"missing dispatcher function {function_name}")
    end = source.find(next_function_name, start + len(function_name))
    if end < 0:
        fail(f"missing dispatcher boundary {next_function_name}")
    clean = strip_comments_preserving_lines(source[start:end])
    labels = list(CASE_RE.finditer(clean))
    result: dict[int, dict[str, Any]] = {}
    for index, label in enumerate(labels):
        address = int(label.group("address"), 16)
        stop = labels[index + 1].start() if index + 1 < len(labels) else len(clean)
        body = clean[label.end() : stop]
        calls = [
            match.group("name")
            for match in CALL_RE.finditer(body)
            if match.group("name") not in CONTROL_CALLS
        ]
        identity = calls[0] if calls else None
        item = {
            "dispatcher_role": role,
            "dispatcher_function": function_name,
            "case_line": line_number(source, start + label.start()),
            "callback_identity": identity,
        }
        previous = result.get(address)
        if previous is not None and previous != item:
            fail(
                f"dispatcher address {hex32(address)} has conflicting {role} cases: "
                f"{previous} vs {item}"
            )
        result[address] = item
    return result


def parse_action_arrays(source: str) -> dict[str, dict[str, Any]]:
    clean = strip_comments_preserving_lines(source)
    arrays: dict[str, dict[str, Any]] = {}
    for match in ARRAY_RE.finditer(clean):
        name = match.group("name")
        runtime_class = int(match.group("class"), 16)
        table_kind = "action" if match.group("kind") == "0" else "constructor"
        declared_size = int(match.group("size"))
        if name in arrays:
            fail(f"duplicate array definition {name}")
        tokens = list(NUMBER_RE.finditer(match.group("body")))
        expected_tokens = declared_size * 4
        if len(tokens) != expected_tokens:  # ubs:ignore — source-token count is not secret material
            fail(
                f"{name}: declared {declared_size} rows but found "
                f"{len(tokens) // 4} rows / {len(tokens)} numeric tokens"
            )
        rows: list[dict[str, Any]] = []
        for slot in range(declared_size):
            values = [
                parse_number(tokens[slot * 4 + offset].group(0))
                for offset in range(4)
            ]
            group_token, selector, address, enabled = values
            first_token = tokens[slot * 4]
            is_sentinel = values == [0, 0, 0, 0]
            rows.append(
                {
                    "array_name": name,
                    "array_declared_size": declared_size,
                    "table_kind": table_kind,
                    "physical_slot": slot,
                    "selector_word_4": selector,
                    "group_token_hex": hex32(group_token),
                    "stored_address_hex": hex32(address) if address else None,
                    "enabled_raw": enabled,
                    "enabled": bool(enabled),
                    "is_null_sentinel": is_sentinel,
                    "source": {
                        "file": TABLE_SOURCE_PATH,
                        "line": line_number(clean, match.start("body") + first_token.start()),
                    },
                }
            )
        sentinel_slots = [
            row["physical_slot"] for row in rows if row["is_null_sentinel"]
        ]
        if sentinel_slots and sentinel_slots != list(range(sentinel_slots[0], declared_size)):
            fail(f"{name}: null sentinel is not a trailing suffix")
        arrays[name] = {
            "name": name,
            "runtime_class": runtime_class,
            "table_kind": table_kind,
            "declared_size": declared_size,
            "source": {
                "file": TABLE_SOURCE_PATH,
                "line": line_number(clean, match.start()),
            },
            "rows": rows,
        }
    expected = {
        f"x_DWORD_D4C52ar_str{runtime_class:X}{kind}"
        for runtime_class in range(1, 16)
        for kind in ("0", "1")
    }
    if set(arrays) != expected:
        fail(
            "dispatch-array coverage mismatch: "
            f"missing={sorted(expected - set(arrays))}, "
            f"extra={sorted(set(arrays) - expected)}"
        )
    return arrays


def parse_class_table(source: str) -> list[dict[str, Any]]:
    clean = strip_comments_preserving_lines(source)
    match = CLASS_TABLE_RE.search(clean)
    if match is None:
        fail("missing std::array<type_D4C52ar, 17> str_D4C48ar definition")
    tokens = list(CLASS_VALUE_RE.finditer(match.group("body")))
    if len(tokens) != 17 * 5:
        fail(f"str_D4C48ar: expected 85 fields, found {len(tokens)}")

    def value(token: re.Match[str]) -> Any:
        raw = token.group(0)
        if raw == "NULL":  # ubs:ignore — parsed source literal comparison is not secret material
            return None
        if raw.startswith("x_DWORD_D4C52ar_str"):
            return raw
        return parse_number(raw)

    rows: list[dict[str, Any]] = []
    for index in range(17):
        row_tokens = tokens[index * 5 : (index + 1) * 5]
        dword_0, dword_4, word_8, action_array, constructor_array = map(value, row_tokens)
        is_sentinel = all(value(token) in (None, 0) for token in row_tokens)
        if index < 16 and dword_4 != index:
            fail(f"str_D4C48ar row {index} has class selector {dword_4!r}")
        if index == 16 and not is_sentinel:
            fail("str_D4C48ar row 16 is not the trailing null sentinel")
        rows.append(
            {
                "class_index": index,
                "dword_0_hex": hex32(dword_0 or 0) if dword_0 is not None else None,
                "dword_4": dword_4,
                "word_8": word_8,
                "action_array": action_array,
                "constructor_array": constructor_array,
                "is_null_sentinel": is_sentinel,
                "source": {
                    "file": TABLE_SOURCE_PATH,
                    "line": line_number(clean, match.start("body") + row_tokens[0].start()),
                },
            }
        )
    return rows


def decorate_row(
    row: dict[str, Any], dispatcher_cases: dict[int, dict[str, Any]]
) -> dict[str, Any]:
    output = dict(row)
    address_hex = row["stored_address_hex"]
    address = int(address_hex, 16) if address_hex else 0
    if row["is_null_sentinel"]:
        resolution = "NULL_SENTINEL"
        callback = None
        dispatcher = None
    else:
        dispatcher = dispatcher_cases.get(address) if address else None
        callback = dispatcher["callback_identity"] if dispatcher else None
        if not row["enabled"]:
            resolution = "DISABLED_WITH_STORED_ADDRESS" if address else "DISABLED_NULL_ADDRESS"
        elif not address:
            resolution = "ENABLED_NULL_ADDRESS"
        elif callback:
            resolution = "ENABLED_RESOLVED"
        elif dispatcher:
            resolution = "ENABLED_DISPATCH_CASE_WITHOUT_CALL"
        else:
            resolution = "ENABLED_ADDRESS_NOT_IN_RECONSTRUCTED_DISPATCH"
    output["callback_identity"] = callback
    output["dispatcher_case_present"] = dispatcher is not None
    output["dispatcher_case_line"] = dispatcher["case_line"] if dispatcher else None
    output["dispatcher_function"] = dispatcher["dispatcher_function"] if dispatcher else None
    output["resolution_status"] = resolution
    return output


def source_behavior_keys(classes: list[dict[str, Any]]) -> list[dict[str, Any]]:
    keys: dict[tuple[int, int], dict[str, Any]] = {}
    for item in classes:
        runtime_class = item["class_index"]
        for row in item["action_rows"]:
            if row["is_null_sentinel"] or not row["enabled"] or not row["stored_address_hex"]:
                continue
            selector = row["selector_word_4"]
            key = (runtime_class, selector)
            keys[key] = {
                "class": runtime_class,
                "action": selector,
                "key": f"{runtime_class}:{selector}",
                "callback_identity": row["callback_identity"],
                "stored_address_hex": row["stored_address_hex"],
                "resolution_status": row["resolution_status"],
                "source": row["source"],
            }
    return [keys[key] for key in sorted(keys)]


def extract(repo_root: Path | None = None) -> dict[str, Any]:
    root = repo_root or repo_root_from_here()
    table_source, table_sha256 = read_pinned_source(root, TABLE_SOURCE_PATH)
    dispatch_source, dispatch_sha256 = read_pinned_source(root, DISPATCH_SOURCE_PATH)
    arrays = parse_action_arrays(table_source)
    classes = parse_class_table(table_source)
    class_table_clean = strip_comments_preserving_lines(table_source)
    class_table_match = CLASS_TABLE_RE.search(class_table_clean)
    if class_table_match is None:
        fail("missing class table while recording its declaration line")
    class_table_declaration_line = line_number(class_table_clean, class_table_match.start())
    action_cases = parse_dispatcher_cases(
        dispatch_source,
        "void pre_sub_4A190_0x6E8E",
        "type_entity_0x6E8E* pre_sub_4A190_axis_3d",
        "action",
    )
    constructor_cases = parse_dispatcher_cases(
        dispatch_source,
        "type_entity_0x6E8E* pre_sub_4A190_axis_3d",
        "type_entity_0x6E8E* IfSubtypeCallCreatingManaSphere_4A190",
        "constructor",
    )

    expected_class_arrays = {
        f"x_DWORD_D4C52ar_str{index:X}{kind}"
        for index in range(1, 16)
        for kind in ("0", "1")
    }
    for item in classes[1:16]:
        if item["action_array"] not in expected_class_arrays:
            fail(f"class {item['class_index']} has unexpected action array {item['action_array']!r}")
        if item["constructor_array"] not in expected_class_arrays:
            fail(
                f"class {item['class_index']} has unexpected constructor array "
                f"{item['constructor_array']!r}"
            )
        action_array = arrays[item["action_array"]]
        constructor_array = arrays[item["constructor_array"]]
        item["action_declared_size"] = action_array["declared_size"]
        item["constructor_declared_size"] = constructor_array["declared_size"]
        item["action_rows"] = [decorate_row(row, action_cases) for row in action_array["rows"]]
        item["constructor_rows"] = [
            decorate_row(row, constructor_cases) for row in constructor_array["rows"]
        ]
        item["action_non_sentinel_count"] = sum(
            not row["is_null_sentinel"] for row in item["action_rows"]
        )
        item["constructor_non_sentinel_count"] = sum(
            not row["is_null_sentinel"] for row in item["constructor_rows"]
        )
    classes[0]["action_rows"] = []
    classes[0]["constructor_rows"] = []
    classes[16]["action_rows"] = []
    classes[16]["constructor_rows"] = []
    for item in classes:
        item.setdefault("action_declared_size", 0)
        item.setdefault("constructor_declared_size", 0)
        item.setdefault("action_non_sentinel_count", 0)
        item.setdefault("constructor_non_sentinel_count", 0)

    all_action_rows = [row for item in classes for row in item["action_rows"]]
    all_constructor_rows = [row for item in classes for row in item["constructor_rows"]]
    source_keys = source_behavior_keys(classes)
    return {
        "schema": SCHEMA,
        "source": {
            "commit": PINNED_COMMIT,
            "read_mode": "git_show_object",
            "class_table": {
                "path": TABLE_SOURCE_PATH,
                "sha256": table_sha256,
                "definition_line": class_table_declaration_line,
            },
            "dispatch_source": {
                "path": DISPATCH_SOURCE_PATH,
                "sha256": dispatch_sha256,
            },
        },
        "class_count": len(classes),
        "classes": classes,
        "source_behavior_keys": source_keys,
        "summary": {
            "class_rows_including_sentinel": len(classes),
            "class_sentinel_rows": sum(item["is_null_sentinel"] for item in classes),
            "action_rows_including_sentinels": len(all_action_rows),
            "action_non_sentinel_rows": sum(not row["is_null_sentinel"] for row in all_action_rows),
            "constructor_rows_including_sentinels": len(all_constructor_rows),
            "constructor_non_sentinel_rows": sum(
                not row["is_null_sentinel"] for row in all_constructor_rows
            ),
            "action_callback_cases": len(action_cases),
            "constructor_callback_cases": len(constructor_cases),
            "source_behavior_key_count": len(source_keys),
        },
    }


def json_bytes(document: dict[str, Any]) -> bytes:
    return (json.dumps(document, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode(
        "utf-8"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=None)
    parser.add_argument("--output", type=Path, default=None)
    args = parser.parse_args(argv)
    try:
        payload = json_bytes(extract(args.repo_root))
    except ValueError as exc:
        print(f"extract_registry.py: error: {exc}", file=sys.stderr)
        return 2
    if args.output is None:
        sys.stdout.buffer.write(payload)
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
