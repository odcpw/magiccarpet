#!/usr/bin/env python3
"""Positive and mutation-negative tests for validate_level1_matrix.py."""

from __future__ import annotations

import collections
import copy
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any, Callable
from unittest import mock

TOOLS_DIR = Path(__file__).resolve().parent
WORKSPACE_ROOT = TOOLS_DIR.parent
CANONICAL_MATRIX = (
    WORKSPACE_ROOT / "level1-conformance" / "level1-conformance-matrix.json"
)
QUALIFICATION_POLICY = (
    WORKSPACE_ROOT / "level1-conformance" / "plan29-final-qualification-policy.json"
)
sys.path.insert(0, str(TOOLS_DIR))

import validate_level1_matrix as validator  # noqa: E402


class Level1MatrixValidatorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.inventory = validator.capture_cargo_test_inventory(WORKSPACE_ROOT)
        cls.canonical = validator.load_matrix(CANONICAL_MATRIX)

    def validate_temporary(self, document: dict[str, Any]) -> dict[str, Any]:
        with tempfile.TemporaryDirectory(prefix="level1-matrix-test-") as directory:
            path = Path(directory) / "matrix.json"
            path.write_text(
                json.dumps(document, indent=2) + "\n",
                encoding="utf-8",
            )
            return validator.validate_file(path, self.inventory)

    def assert_mutation_rejected(
        self,
        mutate: Callable[[dict[str, Any]], None],
        message_fragment: str,
    ) -> None:
        document = copy.deepcopy(self.canonical)
        mutate(document)
        with self.assertRaisesRegex(
            validator.ValidationError, message_fragment
        ):
            self.validate_temporary(document)

    @staticmethod
    def make_one_row_non_exact(document: dict[str, Any]) -> dict[str, Any]:
        row = document["rows"][0]
        row["classification"] = "Partial"
        row["mismatch_detail"] = ["synthetic mutation for validator coverage"]
        row["plan_item"] = "PLAN-01"
        document["plan_items"][0]["owned_rows"] = [row["id"]]
        totals = document["meta"]["totals"]
        totals["by_classification"]["Exact"] -= 1
        totals["by_classification"]["Partial"] += 1
        subsystem = totals["by_subsystem"][row["subsystem"]]
        subsystem["Exact"] -= 1
        subsystem["Partial"] = subsystem.get("Partial", 0) + 1
        return row

    def validate_temporary_audio_ledger(self, document: dict[str, Any]) -> None:
        with tempfile.TemporaryDirectory(prefix="level1-audio-ledger-test-") as directory:
            path = Path(directory) / "audio-ledger.json"
            path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
            path.chmod(0o644)
            with mock.patch.object(validator, "AUDIO_LEDGER_PATH", path):
                validator.validate_file(CANONICAL_MATRIX, self.inventory)

    def assert_correction_contract(self, document: dict[str, Any]) -> None:
        rows = {row["id"]: row for row in document["rows"]}
        scheduler = rows["SCH-004"]
        self.assertIn("DrawAndEventsInGame_47560", scheduler["source"]["symbols"])
        self.assertNotIn("GameRender_28870", scheduler["source"]["symbols"])
        self.assertIn("31724", scheduler["source"]["lines"])
        self.assertIn("31796-31815", scheduler["source"]["lines"])

        rng = rows["RNG-001"]
        rng_contract = json.dumps(
            {
                "symbols": rng["source"]["symbols"],
                "lines": rng["source"]["lines"],
                "behavior": rng["behavior_ordered"],
                "negative": rng["negative_coverage_required"],
            }
        )
        for symbol in (
            "GenerateLevelMap",
            "ApplyEvents",
            "sub_232C0",
            "sub_28FF0",
            "sub_31120",
            "SetParticleSmoke3B_4E9E0",
            "SetParticleSmoke3C_4EA20",
            "sub_4EA60",
            "UpdateEntities_57730",
            "sub_58630",
            "AddArcherArrow_672E0",
            "sub_5D530",
            "PrepareEventSound_6E450",
        ):
            self.assertIn(symbol, rng["source"]["symbols"])
        for token in (
            "EXC-C5-MODELS",
            "EXC-C10-REST",
            "EXC-MULTIPLAYER",
        ):
            self.assertIn(token, rng_contract)

        audio_tests = {
            binding["test"] for binding in rows["AUD-002"]["evidence_bindings"]
        }
        self.assertIn(
            "tests::mission_audio_census_is_unique_complete_and_compiler_bound",
            audio_tests,
        )
        self.assertIn(
            "native_world::tests::level_one_package_excludes_the_family_four_periodic_sound_branch",
            audio_tests,
        )

    def test_canonical_matrix_passes(self) -> None:
        self.assert_correction_contract(self.canonical)
        summary = self.validate_temporary(copy.deepcopy(self.canonical))
        self.assertEqual(
            summary,
            {
                "ok": True,
                "rows": 182,
                "exact_rows": 182,
                "non_exact_rows": 0,
                "exclusions": 29,
                "plan_items": 22,
                "authored_indices": 308,
                "dispatch_slots": 845,
                "cargo_tests": 481,
                "audio_calls": 119,
                "audio_reachable_calls": 46,
                "audio_excluded_calls": 73,
            },
        )
        closed = validator.require_closed(CANONICAL_MATRIX, self.inventory)
        self.assertTrue(closed["closed"])
        self.assertEqual(closed["exact_rows"], 182)

    def test_require_closed_rejects_non_exact_row(self) -> None:
        document = copy.deepcopy(self.canonical)
        self.make_one_row_non_exact(document)
        with tempfile.TemporaryDirectory(prefix="level1-matrix-closed-") as directory:
            path = Path(directory) / "matrix.json"
            path.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
            with self.assertRaisesRegex(
                validator.ValidationError, "requires every one of the 182 rows to be Exact"
            ):
                validator.require_closed(path, self.inventory)

    def test_require_closed_rejects_duplicate_row_id(self) -> None:
        document = copy.deepcopy(self.canonical)
        document["rows"][1]["id"] = document["rows"][0]["id"]
        with self.assertRaisesRegex(validator.ValidationError, "duplicate ID"):
            self.validate_temporary(document)

    def test_require_closed_rejects_wrong_pin(self) -> None:
        document = copy.deepcopy(self.canonical)
        document["meta"]["remc2_commit"] = "0" * 40
        with self.assertRaisesRegex(validator.ValidationError, "pinned remc2 commit"):
            self.validate_temporary(document)

    def test_audio_ledger_mutations_are_rejected(self) -> None:
        ledger = validator.load_matrix(validator.AUDIO_LEDGER_PATH)

        def swap_first_two_tags(document: dict[str, Any]) -> None:
            reachable = [
                record["classification"]
                for record in document["records"]
                if record["classification"]["disposition"] == "reachable"
            ]
            first, second = reachable[:2]
            first_tag = first["proposed_distinct_production_tag"]
            second_tag = second["proposed_distinct_production_tag"]
            first["proposed_distinct_production_tag"] = second_tag
            second["proposed_distinct_production_tag"] = first_tag

        cases: list[tuple[str, Callable[[dict[str, Any]], None], str]] = [
            (
                "omission",
                lambda document: document["records"].pop(),
                "expected 119 source calls",
            ),
            (
                "duplicate identity",
                lambda document: document["records"][1].__setitem__(
                    "stable_occurrence_hash_sha256",
                    document["records"][0]["stable_occurrence_hash_sha256"],
                ),
                "fresh pinned extraction disagrees",
            ),
            (
                "misowned source tag",
                swap_first_two_tags,
                "compiled tag-to-occurrence ownership map disagrees",
            ),
            (
                "preprocessor drift",
                lambda document: document["records"][-1].__setitem__(
                    "preprocessor_context", []
                ),
                "fresh pinned extraction disagrees",
            ),
            (
                "record move",
                lambda document: document["records"].__setitem__(
                    slice(0, 2), reversed(document["records"][:2])
                ),
                "fresh pinned extraction disagrees",
            ),
            (
                "anchor disagreement",
                lambda document: document["records"][0].__setitem__(
                    "pinned_line", document["records"][0]["pinned_line"] + 1
                ),
                "fresh pinned extraction disagrees",
            ),
            (
                "unaccepted classification status",
                lambda document: document["classification_scope_profile"].__setitem__(
                    "status", "proposed"
                ),
                "classifications are not accepted closure evidence",
            ),
            (
                "missing classification proof",
                lambda document: document["records"][0]["classification"].__setitem__(
                    "producer_or_profile_proof", ""
                ),
                "producer_or_profile_proof: expected non-empty string",
            ),
            (
                "platform-state drift",
                lambda document: document["records"][0].__setitem__(
                    "canonical_linux_x64_preprocessor_state",
                    "inactive (#else of defined(__linux__) || defined(__APPLE__))",
                ),
                "platform state disagrees",
            ),
            (
                "swapped exclusion evidence",
                lambda document: document["records"][52].__setitem__(
                    "exclusion_registry_evidence",
                    copy.deepcopy(document["records"][68]["exclusion_registry_evidence"]),
                ),
                "does not match the record exclusion",
            ),
        ]
        for name, mutate, message in cases:
            with self.subTest(name=name):
                document = copy.deepcopy(ledger)
                mutate(document)
                with self.assertRaisesRegex(validator.ValidationError, message):
                    self.validate_temporary_audio_ledger(document)

    def test_canonical_audio_ledger_preserves_shared_semantic_sites(self) -> None:
        ledger = validator.load_matrix(validator.AUDIO_LEDGER_PATH)
        records = {record["pinned_line"]: record for record in ledger["records"]}
        self.assertEqual(
            records[22749]["classification"]["current_rust_tag"],
            ["MissionAudioSite::FireballImpact", "MissionAudioSite::AmbientChildStart"],
        )
        self.assertEqual(
            records[60592]["classification"]["current_rust_tag"],
            [
                "MissionAudioSite::ExitAccelerationPhase6",
                "MissionAudioSite::ExitAccelerationPhase8",
            ],
        )
        self.assertEqual(
            records[44280]["classification"]["current_rust_tag"],
            [
                "MissionAudioSite::FireballLaunch",
                "MissionAudioSite::CastleDefenseFirstShot",
            ],
        )
        validator.validate_audio_call_ledger(self.canonical)

    def test_correction_contract_rejects_invented_scheduler_and_incomplete_rng_census(self) -> None:
        invented = copy.deepcopy(self.canonical)
        scheduler = next(row for row in invented["rows"] if row["id"] == "SCH-004")
        scheduler["source"]["symbols"].remove("DrawAndEventsInGame_47560")
        scheduler["source"]["symbols"].append("GameRender_28870")
        with self.assertRaises(AssertionError):
            self.assert_correction_contract(invented)

        incomplete = copy.deepcopy(self.canonical)
        rng = next(row for row in incomplete["rows"] if row["id"] == "RNG-001")
        rng["source"]["symbols"].remove("sub_58630")
        with self.assertRaises(AssertionError):
            self.assert_correction_contract(incomplete)

        audio_incomplete = copy.deepcopy(self.canonical)
        audio = next(
            row for row in audio_incomplete["rows"] if row["id"] == "AUD-002"
        )
        audio["evidence_bindings"] = [
            binding
            for binding in audio["evidence_bindings"]
            if binding["test"]
            != "tests::mission_audio_census_is_unique_complete_and_compiler_bound"
        ]
        with self.assertRaises(AssertionError):
            self.assert_correction_contract(audio_incomplete)

    def test_shape_valid_unknown_cargo_test_binding_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            document["rows"][0]["evidence_bindings"][0]["test"] = (
                "native_v2::tests::shape_valid_but_nonexistent"
            )

        self.assert_mutation_rejected(mutate, "does not resolve exactly once")

    def test_shape_valid_unknown_unity_test_binding_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            row = next(row for row in document["rows"] if row["id"] == "PRS-001")
            binding = next(
                item for item in row["evidence_bindings"]
                if item["kind"] == "unity-test"
            )
            binding["test"] = (
                "MagicCarpet.Native.Tests.NativeSessionEditModeTests."
                "ShapeValidButNonexistent"
            )

        self.assert_mutation_rejected(mutate, "unity-test binding .* does not resolve")

    def test_shape_valid_unknown_python_test_binding_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            row = next(row for row in document["rows"] if row["id"] == "PRS-001")
            binding = next(
                item for item in row["evidence_bindings"]
                if item["kind"] == "python-test"
            )
            binding["test"] = (
                "test_cache.OwnerCacheTests.test_shape_valid_but_nonexistent"
            )

        self.assert_mutation_rejected(mutate, "python-test binding .* does not resolve")

    def test_dangling_plan_reference_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            row = self.make_one_row_non_exact(document)
            row["plan_item"] = "PLAN-DOES-NOT-EXIST"

        self.assert_mutation_rejected(mutate, "dangling plan_item reference")

    def test_duplicate_authored_census_index_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            indices = document["authored_census"]["nonzero_indices"]
            indices.append(indices[0])

        self.assert_mutation_rejected(mutate, "duplicate values")

    def test_missing_authored_census_index_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            document["authored_census"]["nonzero_indices"].pop()

        self.assert_mutation_rejected(mutate, "expected exactly 308 indices")

    def test_duplicate_dispatch_slot_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            slots = document["dispatch_census"]["slots"]
            slots[-1] = slots[0]

        self.assert_mutation_rejected(mutate, "duplicate values")

    def test_missing_dispatch_slot_mapping_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            mapping = document["dispatch_census"]["slot_to_exclusion"]
            mapping.pop(next(iter(mapping)))

        self.assert_mutation_rejected(mutate, "do not exactly partition slots")

    def test_bad_dispatch_exclusion_mapping_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            mapping = document["dispatch_census"]["slot_to_exclusion"]
            mapping[next(iter(mapping))] = "EXC-DOES-NOT-EXIST"

        self.assert_mutation_rejected(mutate, "unknown exclusion")

    def test_plan_dependency_cycle_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            plans = {plan["id"]: plan for plan in document["plan_items"]}
            plans["PLAN-01"]["depends_on"] = ["PLAN-02"]
            plans["PLAN-02"]["depends_on"] = ["PLAN-01"]

        self.assert_mutation_rejected(mutate, "plan dependency cycle")

    def test_exact_row_without_executable_evidence_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            row = next(
                row for row in document["rows"] if row["classification"] == "Exact"
            )
            row["evidence_bindings"] = []

        self.assert_mutation_rejected(mutate, "no resolving executable rust-test")

    def test_non_exact_row_without_owned_rows_entry_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            row = self.make_one_row_non_exact(document)
            owner = next(
                plan
                for plan in document["plan_items"]
                if plan["id"] == row["plan_item"]
            )
            owner["owned_rows"].remove(row["id"])

        self.assert_mutation_rejected(mutate, "not a bijection")

    def test_non_exact_row_with_duplicate_ownership_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            row = self.make_one_row_non_exact(document)
            other = next(
                plan
                for plan in document["plan_items"]
                if plan["id"] != row["plan_item"]
            )
            other["owned_rows"].append(row["id"])

        self.assert_mutation_rejected(mutate, "duplicate ownership")

    def test_exact_row_ownership_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            row = next(
                row for row in document["rows"] if row["classification"] == "Exact"
            )
            document["plan_items"][0]["owned_rows"].append(row["id"])

        self.assert_mutation_rejected(mutate, "Exact row .* must not be owned")

    def test_inconsistent_meta_total_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            document["meta"]["totals"]["rows"] += 1

        self.assert_mutation_rejected(mutate, "expected recomputed value 182")

    def test_unknown_field_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            document["rows"][0]["quiet_default"] = True

        self.assert_mutation_rejected(mutate, "unknown fields")
        self.assert_mutation_rejected(
            lambda document: next(
                row for row in document["rows"] if row["id"] == "DSP-03-02"
            )["source"]["related"][0].__setitem__("quiet_default", True),
            "unknown fields",
        )
        self.assert_mutation_rejected(
            lambda document: next(
                row for row in document["rows"] if row["id"] == "DSP-03-02"
            )["source"].__setitem__("related", {}),
            "expected array",
        )

    def test_missing_field_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            del document["rows"][0]["confidence"]

        self.assert_mutation_rejected(mutate, "missing fields")
        self.assert_mutation_rejected(
            lambda document: next(
                row for row in document["rows"] if row["id"] == "DSP-03-02"
            )["source"]["related"][0].__delitem__("path"),
            "missing fields",
        )

    def test_weakly_typed_integer_is_rejected(self) -> None:
        def mutate(document: dict[str, Any]) -> None:
            document["dispatch_census"]["slot_count"] = "845"

        self.assert_mutation_rejected(mutate, "expected integer")

    def test_duplicate_json_field_is_rejected(self) -> None:
        canonical_text = CANONICAL_MATRIX.read_text(encoding="utf-8")
        duplicate_text = canonical_text.replace(
            '"schema": {',
            '"schema": null,\n  "schema": {',
            1,
        )
        with tempfile.TemporaryDirectory(prefix="level1-matrix-test-") as directory:
            path = Path(directory) / "matrix.json"
            path.write_text(duplicate_text, encoding="utf-8")
            with self.assertRaisesRegex(
                validator.ValidationError, "duplicate field 'schema'"
            ):
                validator.validate_file(path, self.inventory)

    def test_ambiguous_live_test_inventory_is_rejected(self) -> None:
        inventory = collections.Counter(self.inventory)
        bound_name = self.canonical["rows"][0]["evidence_bindings"][0]["test"]
        inventory[bound_name] = 2
        with self.assertRaisesRegex(
            validator.ValidationError, "does not resolve exactly once"
        ):
                validator.validate_document(copy.deepcopy(self.canonical), inventory)

    def test_one_run_policy_and_receipt_mutations_fail_closed(self) -> None:
        policy = validator.load_matrix(QUALIFICATION_POLICY)
        with tempfile.TemporaryDirectory(prefix="plan29-policy-test-") as directory:
            path = Path(directory) / "policy.json"
            path.write_text(json.dumps(policy, indent=2) + "\n", encoding="utf-8")
            policy_file_sha = validator.validate_qualification_policy(
                path,
                policy["behavior_contract"]["file_sha256"],
                policy["behavior_contract"]["self_sha256"],
            )

            identity = {"matrix_sha256": "a" * 64}
            run = {
                "schema": "mc2-plan29-clean-run/v1",
                "run_id": "run-a-test",
                "run_root": "/private/run-a",
                "clean": {
                    "cargo_target_isolated": True,
                    "unity_library_isolated": True,
                    "cache_isolated": True,
                    "preexisting_artifacts_rejected": True,
                },
                "gates": {
                    key: "PASS"
                    for key in {
                        "rust", "c_abi", "sim_cli", "matrix_closed", "cache",
                        "unity_edit_mode", "unity_play_mode", "victory", "failure",
                        "restart", "replay", "privacy", "provenance", "presentation",
                    }
                },
                "identity": identity,
                "semantic_digests": {
                    key: "b" * 64
                    for key in (
                        "reset", "victory", "failure", "restart", "replay",
                        "observation_mask", "unity_checkpoints", "full_run",
                    )
                },
                "status": "PASS",
            }
            run["receipt_sha256"] = validator.sha256_hex(
                validator.canonical_json_bytes(run)
            )
            receipts = {
                "qualification_policy_sha256": policy_file_sha,
                "clean_runs": {
                    "schema": "mc2-plan29-full-gate/v1",
                    "qualification_policy_sha256": policy_file_sha,
                    "exact_run_count": 1,
                    "runs": [run],
                    "run_count_policy": {
                        "required_runs": 1,
                        "completed_runs": 1,
                        "run_b_required": False,
                        "run_b_permitted": False,
                    },
                    "status": "PASS",
                },
            }
            self.assertEqual(
                validator.validate_clean_run(receipts, identity, policy_file_sha),
                ["run-a-test"],
            )

            for count in (0, 2):
                mutated = copy.deepcopy(receipts)
                mutated["clean_runs"]["exact_run_count"] = count
                mutated["clean_runs"]["runs"] = [copy.deepcopy(run) for _ in range(count)]
                with self.subTest(count=count), self.assertRaises(
                    validator.ValidationError
                ):
                    validator.validate_clean_run(mutated, identity, policy_file_sha)

            bad_policy = copy.deepcopy(policy)
            bad_policy["acceptance"]["required_clean_runs"] = 2
            bad_policy_path = Path(directory) / "bad-policy.json"
            bad_policy_path.write_text(
                json.dumps(bad_policy, indent=2) + "\n", encoding="utf-8"
            )
            with self.assertRaises(validator.ValidationError):
                validator.validate_qualification_policy(
                    bad_policy_path,
                    policy["behavior_contract"]["file_sha256"],
                    policy["behavior_contract"]["self_sha256"],
                )


if __name__ == "__main__":
    unittest.main(verbosity=2)
