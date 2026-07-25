#!/usr/bin/env python3
"""Extract every remc2 runtime action and constructor dispatch row.

The normal C++ call graph cannot see calls stored as original addresses in
``str_D4C48ar``. This private mechanical atlas parses all per-class tables,
resolves stored addresses through remc2's two compatibility switch dispatchers,
and retains sentinels, disabled rows, and unresolved addresses as evidence.
"""

from __future__ import annotations

import csv
import json
import re
from collections import Counter
from pathlib import Path


HERE = Path(__file__).resolve().parent
LAB_ROOT = HERE.parents[2]
ENGINE = LAB_ROOT / "upstream/magic-carpet-2-hd/remc2/engine"
TABLE_SOURCE = ENGINE / "EventsFunctions.cpp"
DISPATCH_SOURCE = ENGINE / "Events.cpp"
OUTPUT = HERE / "semantics/generated"
CLAIMS_SOURCE = OUTPUT / "claims.csv"
CREATURE_MODELS_SOURCE = OUTPUT / "creature-models.csv"
CREATURE_ACTION_EXCEPTIONS_SOURCE = OUTPUT / "creature-action-exceptions.csv"
SPELL_FAMILIES_SOURCE = OUTPUT / "spell-families.csv"
MODEL_ANNOTATIONS_SOURCE = OUTPUT / "runtime-model-annotations.csv"
ACTION_ANNOTATIONS_SOURCE = OUTPUT / "runtime-action-annotations.csv"

ARRAY_RE = re.compile(
    r"type_D4C52ar2\s+"
    r"x_DWORD_D4C52ar_str(?P<class>[0-9A-F])(?P<kind>[01])"
    r"\[(?P<size>\d+)\]\s*=\s*\{(?P<body>.*?)\};",
    re.DOTALL,
)
TOKEN_RE = re.compile(r"0x[0-9A-Fa-f]+|\b\d+\b")
CASE_LABEL_RE = re.compile(r"case\s+0x(?P<address>[0-9A-Fa-f]+)\s*:")
CALL_RE = re.compile(
    r"(?:return\s+|(?:[A-Za-z_]\w*\s*=\s*)?)"
    r"(?P<function>[A-Za-z_]\w*)\s*\("
)
CLASS_MODEL_RE = re.compile(r"class=(?P<class>\d+);model=(?P<model>\d+)")


def strip_comments_preserving_lines(text: str) -> str:
    def replace(match: re.Match[str]) -> str:
        return "".join("\n" if char == "\n" else " " for char in match.group(0))

    return re.sub(r"//[^\n]*|/\*.*?\*/", replace, text, flags=re.DOTALL)


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def function_slice(text: str, name: str, next_name: str) -> str:
    start = text.index(name)
    end = text.index(next_name, start + len(name))
    return text[start:end]


def dispatcher_address_map(
    source: str, function_name: str, next_function_name: str
) -> tuple[dict[int, str], set[int]]:
    body = strip_comments_preserving_lines(
        function_slice(source, function_name, next_function_name)
    )
    output: dict[int, str] = {}
    cases: set[int] = set()
    ignored = {
        "if",
        "switch",
        "while",
        "for",
        "mydelay",
        "exit",
        "test_pre_sub_4a190",
    }
    labels = list(CASE_LABEL_RE.finditer(body))
    for index, match in enumerate(labels):
        address = int(match.group("address"), 16)
        cases.add(address)
        end = labels[index + 1].start() if index + 1 < len(labels) else len(body)
        case_body = body[match.end() : end]
        calls = [
            item.group("function")
            for item in CALL_RE.finditer(case_body)
            if item.group("function") not in ignored
        ]
        if calls:
            output[address] = calls[0]
    return output, cases


def parse_tables(source: str) -> list[dict[str, object]]:
    clean = strip_comments_preserving_lines(source)
    rows: list[dict[str, object]] = []
    seen_arrays: set[tuple[int, str]] = set()
    for match in ARRAY_RE.finditer(clean):
        runtime_class = int(match.group("class"), 16)
        table_kind = "action" if match.group("kind") == "0" else "constructor"
        declared_size = int(match.group("size"))
        key = (runtime_class, table_kind)
        if key in seen_arrays:
            raise ValueError(f"duplicate dispatch array for {key}")
        seen_arrays.add(key)

        tokens = list(TOKEN_RE.finditer(match.group("body")))
        if len(tokens) != declared_size * 4:
            raise ValueError(
                f"class {runtime_class} {table_kind}: declared {declared_size} "
                f"rows but parsed {len(tokens)} numeric tokens"
            )
        for slot in range(declared_size):
            group_token, selector, address, enabled = (
                int(tokens[slot * 4 + index].group(0), 0) for index in range(4)
            )
            first_token = tokens[slot * 4]
            absolute_offset = match.start("body") + first_token.start()
            is_sentinel = group_token == selector == address == enabled == 0
            rows.append(
                {
                    "runtime_class": runtime_class,
                    "table_kind": table_kind,
                    "physical_slot": slot,
                    "selector_word_4": selector,
                    "group_token_hex": f"0x{group_token:08X}",
                    "stored_address_hex": f"0x{address:08X}" if address else "",
                    "enabled_raw": enabled,
                    "enabled": bool(enabled),
                    "is_null_sentinel": is_sentinel,
                    "array_name": (
                        f"x_DWORD_D4C52ar_str{runtime_class:X}"
                        f"{'0' if table_kind == 'action' else '1'}"
                    ),
                    "array_declared_size": declared_size,
                    "source_file": "EventsFunctions.cpp",
                    "source_line": line_number(clean, absolute_offset),
                }
            )
    expected = {
        (runtime_class, table_kind)
        for runtime_class in range(1, 16)
        for table_kind in ("action", "constructor")
    }
    if seen_arrays != expected:
        missing = sorted(expected - seen_arrays)
        extra = sorted(seen_arrays - expected)
        raise ValueError(f"dispatch array coverage mismatch; missing={missing}, extra={extra}")
    return rows


def resolution_status(
    row: dict[str, object], resolved_function: str, dispatcher_case_present: bool
) -> str:
    address = str(row["stored_address_hex"])
    enabled = bool(row["enabled"])
    if bool(row["is_null_sentinel"]):
        return "NULL_SENTINEL"
    if not address:
        return "ENABLED_NULL_ADDRESS" if enabled else "DISABLED_NULL_ADDRESS"
    if not enabled:
        return "DISABLED_WITH_STORED_ADDRESS"
    if resolved_function:
        return "ENABLED_RESOLVED"
    if dispatcher_case_present:
        return "ENABLED_DISPATCH_CASE_WITHOUT_CALL"
    return "ENABLED_ADDRESS_NOT_IN_RECONSTRUCTED_DISPATCH"


def write_csv(path: Path, rows: list[dict[str, object]]) -> None:
    if not rows:
        raise ValueError(f"refusing to write empty output: {path}")
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def semantic_overlays() -> tuple[
    dict[tuple[int, int], dict[str, str]],
    dict[tuple[int, int], dict[str, str]],
]:
    """Join only identity spaces whose relationship is statically proven."""
    constructor_claims: dict[tuple[int, int], list[dict[str, str]]] = {}
    for claim in read_csv(CLAIMS_SOURCE):
        match = CLASS_MODEL_RE.search(claim["raw_key"])
        if not match:
            continue
        key = (int(match.group("class")), int(match.group("model")))
        constructor_claims.setdefault(key, []).append(claim)

    confidence_rank = {
        "CONFLICTED": 0,
        "PROVISIONAL": 1,
        "STRONG": 2,
        "EXACT_STATIC": 3,
        "CONFIRMED": 4,
    }
    constructor_overlay: dict[tuple[int, int], dict[str, str]] = {}
    for key, claims in constructor_claims.items():
        labels = sorted({row["proposed_label"] for row in claims})
        claim_ids = sorted({row["claim_id"] for row in claims})
        confidence = max(
            (row["confidence"] for row in claims),
            key=lambda value: confidence_rank.get(value, -1),
        )
        constructor_overlay[key] = {
            "semantic_label": " | ".join(labels),
            "semantic_confidence": confidence,
            "semantic_claim_ids": ";".join(claim_ids),
            "semantic_scope": "constructor class/model identity",
        }
    annotation_keys: set[tuple[int, int]] = set()
    for annotation in read_csv(MODEL_ANNOTATIONS_SOURCE):
        key = (
            int(annotation["runtime_class"]),
            int(annotation["constructor_physical_slot"]),
        )
        if key in annotation_keys:
            raise ValueError(f"duplicate runtime model annotation: {key}")
        annotation_keys.add(key)
        existing = constructor_overlay.get(key)
        if existing is None:
            constructor_overlay[key] = {
                "semantic_label": annotation["semantic_label"],
                "semantic_confidence": annotation["confidence"],
                "semantic_claim_ids": "",
                "semantic_scope": "constructor class/model identity annotation",
            }
            continue
        existing_labels = set(existing["semantic_label"].split(" | "))
        existing_labels.add(annotation["semantic_label"])
        existing["semantic_label"] = " | ".join(sorted(existing_labels))
        if confidence_rank.get(annotation["confidence"], -1) > confidence_rank.get(
            existing["semantic_confidence"], -1
        ):
            existing["semantic_confidence"] = annotation["confidence"]

    action_overlay: dict[tuple[int, int], dict[str, str]] = {}
    for creature in read_csv(CREATURE_MODELS_SOURCE):
        runtime_class = int(creature["runtime_class"])
        model = int(creature["runtime_model"])
        for action in range(
            int(creature["regular_action_start"]),
            int(creature["regular_action_end"]) + 1,
        ):
            action_overlay[(runtime_class, action)] = {
                "semantic_label": (
                    f"{creature['semantic_label']} regular action "
                    f"{action - int(creature['regular_action_start'])}"
                ),
                "semantic_confidence": creature["confidence"],
                "semantic_claim_ids": f"runtime-creature-model-{model:02d}",
                "semantic_scope": "proven class-5 eight-action model band",
            }
    for exception in read_csv(CREATURE_ACTION_EXCEPTIONS_SOURCE):
        action = int(exception["action_index"])
        action_overlay[(5, action)] = {
            "semantic_label": (
                f"class-5 exceptional action {action}: {exception['status']}"
            ),
            "semantic_confidence": "STRONG",
            "semantic_claim_ids": "",
            "semantic_scope": "class-5 action outside regular model bands",
        }
    for family in read_csv(SPELL_FAMILIES_SOURCE):
        family_id = int(family["family_id"])
        for tier_action in range(3):
            action_overlay[(15, family_id * 3 + tier_action)] = {
                "semantic_label": (
                    f"{family['family_name']} spell-holder action {tier_action}"
                ),
                "semantic_confidence": family["confidence"],
                "semantic_claim_ids": f"spell-family-{family_id:02d}",
                "semantic_scope": "proven class-15 family times three action band",
            }
        constructor_overlay[(15, family_id)] = {
            "semantic_label": f"{family['family_name']} spell holder",
            "semantic_confidence": family["confidence"],
            "semantic_claim_ids": (
                f"spell-family-{family_id:02d};runtime-spell-holder"
            ),
            "semantic_scope": "constructor class/model identity",
        }
    action_overlay[(15, 78)] = {
        "semantic_label": "extra class-15 action outside the 26 by 3 family bands",
        "semantic_confidence": "STRONG",
        "semantic_claim_ids": "runtime-spell-holder",
        "semantic_scope": "class-15 action-band exception",
    }
    action_annotation_keys: set[tuple[int, int]] = set()
    for annotation in read_csv(ACTION_ANNOTATIONS_SOURCE):
        key = (
            int(annotation["runtime_class"]),
            int(annotation["action_physical_slot"]),
        )
        if key in action_annotation_keys:
            raise ValueError(f"duplicate runtime action annotation: {key}")
        action_annotation_keys.add(key)
        existing = action_overlay.get(key, {})
        action_overlay[key] = {
            "semantic_label": annotation["semantic_label"],
            "semantic_confidence": annotation["confidence"],
            "semantic_claim_ids": existing.get("semantic_claim_ids", ""),
            "semantic_scope": annotation["annotation_level"],
            "behavior_summary": annotation["behavior_summary"],
            "constructor_or_transition_linkage": annotation[
                "constructor_or_transition_linkage"
            ],
            "semantic_evidence_anchors": annotation["evidence_anchors"],
            "semantic_notes": annotation["notes"],
            "structural_band_label": existing.get("semantic_label", ""),
        }
    return constructor_overlay, action_overlay


def main() -> int:
    table_source = TABLE_SOURCE.read_text(encoding="utf-8")
    dispatch_source = DISPATCH_SOURCE.read_text(encoding="utf-8")
    action_map, action_cases = dispatcher_address_map(
        dispatch_source,
        "void pre_sub_4A190_0x6E8E",
        "type_entity_0x6E8E* pre_sub_4A190_axis_3d",
    )
    constructor_map, constructor_cases = dispatcher_address_map(
        dispatch_source,
        "type_entity_0x6E8E* pre_sub_4A190_axis_3d",
        "type_entity_0x6E8E* IfSubtypeCallCreatingManaSphere_4A190",
    )

    rows = parse_tables(table_source)
    constructor_overlay, action_overlay = semantic_overlays()
    output_rows: list[dict[str, object]] = []
    for row in rows:
        address_text = str(row["stored_address_hex"])
        address = int(address_text, 16) if address_text else 0
        address_map = action_map if row["table_kind"] == "action" else constructor_map
        dispatcher_cases = (
            action_cases if row["table_kind"] == "action" else constructor_cases
        )
        resolved = address_map.get(address, "")
        dispatcher_case_present = address in dispatcher_cases
        constructor_overlay_key = (
            int(row["runtime_class"]),
            int(row["physical_slot"]),
        )
        action_overlay_key = (
            int(row["runtime_class"]),
            int(row["physical_slot"]),
        )
        overlay = {}
        if not row["is_null_sentinel"]:
            overlay = (
                constructor_overlay.get(constructor_overlay_key, {})
                if row["table_kind"] == "constructor"
                else action_overlay.get(action_overlay_key, {})
            )
        output_rows.append(
            {
                **row,
                "resolved_function": resolved,
                "dispatcher_case_present": dispatcher_case_present,
                "resolution_status": resolution_status(
                    row, resolved, dispatcher_case_present
                ),
                "semantic_label": overlay.get("semantic_label", ""),
                "semantic_confidence": overlay.get(
                    "semantic_confidence", "MECHANICAL"
                ),
                "semantic_claim_ids": overlay.get("semantic_claim_ids", ""),
                "semantic_scope": overlay.get(
                    "semantic_scope", "raw dispatch row only"
                ),
                "behavior_summary": overlay.get("behavior_summary", ""),
                "constructor_or_transition_linkage": overlay.get(
                    "constructor_or_transition_linkage", ""
                ),
                "semantic_evidence_anchors": overlay.get(
                    "semantic_evidence_anchors", ""
                ),
                "semantic_notes": overlay.get("semantic_notes", ""),
                "structural_band_label": overlay.get("structural_band_label", ""),
                "semantic_guardrail": (
                    "physical slot, stored selector/address, and switch resolution "
                    "are mechanical; action semantics are joined by physical slot "
                    "and never by guessed model/action equality"
                ),
            }
        )

    output_rows.sort(
        key=lambda row: (
            int(row["runtime_class"]),
            0 if row["table_kind"] == "action" else 1,
            int(row["physical_slot"]),
        )
    )
    status_counts = Counter(str(row["resolution_status"]) for row in output_rows)
    by_class_kind = Counter(
        f"class_{int(row['runtime_class']):02d}_{row['table_kind']}"
        for row in output_rows
    )
    unresolved_enabled = [
        row
        for row in output_rows
        if row["resolution_status"]
        in {
            "ENABLED_NULL_ADDRESS",
            "ENABLED_DISPATCH_CASE_WITHOUT_CALL",
            "ENABLED_ADDRESS_NOT_IN_RECONSTRUCTED_DISPATCH",
        }
    ]
    constructor_rows = [
        {
            "runtime_class": row["runtime_class"],
            "constructor_physical_slot": row["physical_slot"],
            "runtime_model_selector": row["selector_word_4"],
            "enabled": row["enabled"],
            "is_null_sentinel": row["is_null_sentinel"],
            "stored_address_hex": row["stored_address_hex"],
            "dispatcher_case_present": row["dispatcher_case_present"],
            "resolved_constructor_function": row["resolved_function"],
            "resolution_status": row["resolution_status"],
            "semantic_label": row["semantic_label"],
            "semantic_confidence": row["semantic_confidence"],
            "semantic_claim_ids": row["semantic_claim_ids"],
            "source_file": row["source_file"],
            "source_line": row["source_line"],
            "guardrail": (
                "physical slot and selector are mechanical; friendly model identity "
                "appears only when an evidence-backed overlay resolves this exact key"
            ),
        }
        for row in output_rows
        if row["table_kind"] == "constructor"
    ]
    action_rows = [row for row in output_rows if row["table_kind"] == "action"]
    unlabeled_constructor_keys = {
        (int(row["runtime_class"]), int(row["constructor_physical_slot"]))
        for row in constructor_rows
        if not row["is_null_sentinel"] and not row["semantic_label"]
    }
    expected_addressless_keys = {
        *((1, slot) for slot in range(9)),
        *((13, model) for model in range(10)),
    }
    if unlabeled_constructor_keys != expected_addressless_keys:
        raise ValueError(
            "constructor semantic overlay gap changed; "
            f"actual={sorted(unlabeled_constructor_keys)} "
            f"expected={sorted(expected_addressless_keys)}"
        )
    summary = {
        "schema_version": 1,
        "classification": "PRIVATE_ANALYSIS",
        "declared_dispatch_row_denominator": len(output_rows),
        "mechanically_parsed_dispatch_rows": len(output_rows),
        "runtime_classes_with_tables": 15,
        "action_rows_including_sentinels": sum(
            row["table_kind"] == "action" for row in output_rows
        ),
        "constructor_rows_including_sentinels": sum(
            row["table_kind"] == "constructor" for row in output_rows
        ),
        "resolution_status_counts": dict(sorted(status_counts.items())),
        "class_table_row_counts": dict(sorted(by_class_kind.items())),
        "enabled_unresolved_count": len(unresolved_enabled),
        "enabled_unresolved_rows": [
            {
                "runtime_class": row["runtime_class"],
                "table_kind": row["table_kind"],
                "physical_slot": row["physical_slot"],
                "selector_word_4": row["selector_word_4"],
                "stored_address_hex": row["stored_address_hex"],
                "dispatcher_case_present": row["dispatcher_case_present"],
                "source_line": row["source_line"],
                "resolution_status": row["resolution_status"],
            }
            for row in unresolved_enabled
        ],
        "constructor_rows_with_semantic_labels": sum(
            bool(row["semantic_label"]) for row in constructor_rows
        ),
        "constructor_non_sentinel_rows": sum(
            not row["is_null_sentinel"] for row in constructor_rows
        ),
        "constructor_non_sentinel_rows_with_semantic_labels": sum(
            not row["is_null_sentinel"] and bool(row["semantic_label"])
            for row in constructor_rows
        ),
        "constructor_non_sentinel_rows_without_semantic_labels": [
            {
                "runtime_class": row["runtime_class"],
                "constructor_physical_slot": row["constructor_physical_slot"],
                "runtime_model_selector": row["runtime_model_selector"],
                "resolution_status": row["resolution_status"],
            }
            for row in constructor_rows
            if not row["is_null_sentinel"] and not row["semantic_label"]
        ],
        "action_non_sentinel_rows": sum(
            not row["is_null_sentinel"] for row in action_rows
        ),
        "action_non_sentinel_rows_with_semantic_labels": sum(
            not row["is_null_sentinel"] and bool(row["semantic_label"])
            for row in action_rows
        ),
        "action_non_sentinel_rows_with_individual_behavior": sum(
            not row["is_null_sentinel"]
            and row["semantic_scope"] == "INDIVIDUAL_BEHAVIOR"
            for row in action_rows
        ),
        "action_non_sentinel_rows_with_structural_status_only": sum(
            not row["is_null_sentinel"]
            and row["semantic_scope"] == "STRUCTURAL_STATUS"
            for row in action_rows
        ),
        "outputs": {
            "runtime_dispatch_rows": "runtime-dispatch-rows.csv",
            "runtime_constructor_registry": "runtime-constructor-registry.csv",
            "runtime_action_annotations": "runtime-action-annotations.csv",
        },
        "guardrail": (
            "every non-sentinel action row is classified; individual behavior "
            "annotations override only the display label while retaining the "
            "proven structural model/family band and claim linkage"
        ),
    }

    OUTPUT.mkdir(parents=True, exist_ok=True)
    write_csv(OUTPUT / "runtime-dispatch-rows.csv", output_rows)
    write_csv(OUTPUT / "runtime-constructor-registry.csv", constructor_rows)
    (OUTPUT / "dispatch-atlas-summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    for path in (
        OUTPUT / "runtime-dispatch-rows.csv",
        OUTPUT / "dispatch-atlas-summary.json",
    ):
        path.chmod(0o600)
    Path(__file__).chmod(0o700)
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
