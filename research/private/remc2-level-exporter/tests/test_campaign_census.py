from __future__ import annotations

import copy
import importlib.util
from pathlib import Path

import pytest

SCRIPT = Path(__file__).parents[1] / "scripts" / "campaign_census.py"
SPEC = importlib.util.spec_from_file_location("campaign_census", SCRIPT)
assert SPEC and SPEC.loader
census = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(census)


def test_level_selector_is_exact_remaining_campaign() -> None:
    assert census.level_list("3-24") == list(range(3, 25))
    with pytest.raises(census.CensusError, match="exactly Levels 3..24"):
        census.level_list("2-24")
    with pytest.raises(census.CensusError, match="exactly Levels 3..24"):
        census.level_list("3-23")


def test_canonical_hash_and_closure_are_declaration_order_independent() -> None:
    left = {"rows": [{"class": 5, "action": 9}, {"class": 2, "action": 1}], "facts": {"b": 2, "a": 1}}
    right = {"facts": {"a": 1, "b": 2}, "rows": [{"class": 5, "action": 9}, {"class": 2, "action": 1}]}
    assert census.canonical_sha256(left) == census.canonical_sha256(right)

    graph = {"root": ["handler-b"], "handler-b": ["handler-c"], "handler-c": []}
    expected = ["handler-b", "handler-c", "root"]
    assert census.closure_fixpoint(["root"], graph) == expected
    census.validate_declared_closure(["root"], graph, expected)


def test_missing_transitive_handler_is_rejected() -> None:
    graph = {"root": ["handler-b"], "handler-b": ["handler-c"]}
    with pytest.raises(census.CensusError, match="transitive fixpoint"):
        census.validate_declared_closure(["root"], graph, ["handler-b", "root"])


def _twin_fixture() -> dict:
    return {
        "external_level": 3,
        "identity": {
            "manifest_sha256": "a" * 64,
            "package_sha256": "b" * 64,
            "file_sha256": {"runtime.bin": "c" * 64},
            "source_upstream_commit": census.COMMIT,
            "source_compiled_sha256": "d" * 64,
            "owner_files_sha256": "e" * 64,
        },
        "source_owner_files": [{"logical_path": "CD_Files/DATA/SPELLS.DAT", "bytes": 1, "sha256": "f" * 64}],
        "observed": {"required_source_facts": [{"fact_id": "collision-mask-bank", "member_sha256": "1" * 64}]},
    }


def test_package_identity_mutation_is_rejected() -> None:
    first = _twin_fixture()
    second = copy.deepcopy(first)
    second["identity"]["package_sha256"] = "0" * 64
    with pytest.raises(census.CensusError, match="identities differ"):
        census.compare_twins(first, second)


def test_source_fact_mutation_is_rejected() -> None:
    first = _twin_fixture()
    second = copy.deepcopy(first)
    second["observed"]["required_source_facts"][0]["member_sha256"] = "0" * 64
    with pytest.raises(census.CensusError, match="observations differ"):
        census.compare_twins(first, second)


def test_authored_tuple_preserves_source_sentinel_and_field_vocabulary() -> None:
    raw = bytearray(26116)
    raw[6] = 0
    offset = census.AUTHORED_OFFSET
    raw[offset : offset + 20] = bytes.fromhex("0a002d0064006500ffff00000000110000000000")
    rows, tuples = census.parse_authored(bytes(raw), 3)
    assert rows[0]["class"] == 10
    assert rows[0]["action"] == 255
    assert rows[0]["model"] == 17
    assert rows[0]["subtype"] == 45
    assert rows[0]["source_fields"]["DisId"] == -1
    assert list(tuples) == [(10, 255, 17, 45)]
