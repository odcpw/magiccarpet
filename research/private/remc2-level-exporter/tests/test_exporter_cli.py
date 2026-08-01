#!/usr/bin/env python3
"""Focused production-path tests for the owner-facing level selector.

The parser lives in ``src/level_selection.h`` and is included by the exporter
itself. This test compiles a tiny harness against that same header, then (when
the built exporter is available) checks that invalid CLI input fails before an
output or staging directory can appear.
"""

from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import json


ROOT = Path(__file__).resolve().parents[4]
EXPORTER_SOURCE = ROOT / "research/private/remc2-level-exporter/src"


def compile_selector_harness(directory: Path) -> Path:
    source = directory / "selector_harness.cpp"
    source.write_text(
        """
#include \"level_selection.h\"
#include <iostream>
#include <string>

int main(int argc, char** argv) {
    for (int i = 1; i < argc; ++i) {
        try {
            const auto selection = mc2_exporter::parse_external_level(argv[i]);
            std::cout << selection.external << ":" << selection.internal << "\\n";
        } catch (const std::exception& error) {
            std::cerr << error.what() << "\\n";
            return 1;
        }
    }
}
""",
        encoding="utf-8",
    )
    binary = directory / "selector_harness"
    compiler = os.environ.get("CXX", "c++")
    subprocess.run(
        [compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", "-I", str(EXPORTER_SOURCE), str(source), "-o", str(binary)],
        check=True,
        capture_output=True,
        text=True,
    )
    return binary


def test_selector_mapping_and_rejection() -> None:
    with tempfile.TemporaryDirectory(prefix="mc2-exporter-selector-") as temporary:
        binary = compile_selector_harness(Path(temporary))
        mapped = subprocess.run(
            [str(binary), "1", "2", "25"],
            check=True,
            capture_output=True,
            text=True,
        )
        assert mapped.stdout.splitlines() == ["1:0", "2:1", "25:24"]

        for invalid in ("", "0", "26", "-1", "+1", "1x", " 2", "2 ", "999999999999999999999"):
            rejected = subprocess.run(
                [str(binary), invalid],
                capture_output=True,
                text=True,
            )
            assert rejected.returncode != 0, invalid


def test_invalid_cli_has_no_partial_output() -> None:
    exporter = Path(
        os.environ.get(
            "MC2_LEVEL_EXPORTER_BIN",
            "/tmp/magiccarpet-remc2-level-exporter-build/remc2-level-exporter",
        )
    )
    if not exporter.is_file():
        print(
            "SKIP production CLI negative tests: exporter binary is absent; "
            "set MC2_LEVEL_EXPORTER_BIN after a clean pinned build",
        )
        return
    owner_root = Path(
        os.environ.get(
            "MC2_OWNER_ROOT",
            str(ROOT / "research/private/remc2-native-runtime"),
        )
    )
    with tempfile.TemporaryDirectory(prefix="mc2-exporter-invalid-") as temporary:
        parent = Path(temporary)
        for invalid in ("0", "26", "-1", "1x"):
            output = parent / f"output-{invalid.replace('-', 'negative-')}"
            result = subprocess.run(
                [str(exporter), "--owner-root", str(owner_root), "--output", str(output), "--level", invalid],
                capture_output=True,
                text=True,
            )
            assert result.returncode != 0, (invalid, result.stdout, result.stderr)
            assert not output.exists(), invalid
            assert not Path(f"{output}.staging").exists(), invalid

        duplicate_output = parent / "duplicate"
        duplicate = subprocess.run(
            [
                str(exporter),
                "--owner-root",
                str(owner_root),
                "--output",
                str(duplicate_output),
                "--level",
                "1",
                "--level",
                "1",
            ],
            capture_output=True,
            text=True,
        )
        assert duplicate.returncode != 0
        assert not duplicate_output.exists()
        assert not Path(f"{duplicate_output}.staging").exists()


def _tree_bytes(root: Path) -> dict[str, bytes]:
    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def test_deterministic_level_profiles() -> None:
    """Run the full production exporter gate only when explicitly requested.

    Four native captures are intentionally opt-in because each capture starts
    remc2 and can be long. The opt-in receipt compares the complete trees for
    default/explicit Level 1 and for two independent Level 2 exports.
    """

    exporter = Path(
        os.environ.get(
            "MC2_LEVEL_EXPORTER_BIN",
            "/tmp/magiccarpet-remc2-level-exporter-build/remc2-level-exporter",
        )
    )
    if os.environ.get("MC2_LEVEL_EXPORTER_RUN_FULL") != "1":
        print("SKIP full exporter determinism gate: set MC2_LEVEL_EXPORTER_RUN_FULL=1")
        return
    if not exporter.is_file():
        raise AssertionError(
            "MC2_LEVEL_EXPORTER_RUN_FULL=1 requires MC2_LEVEL_EXPORTER_BIN from a "
            "clean pinned remc2 build"
        )
    owner_root = Path(
        os.environ.get(
            "MC2_OWNER_ROOT",
            str(ROOT / "research/private/remc2-native-runtime"),
        )
    )
    generated_root = ROOT / "research/private/generated"
    generated_root.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp(prefix="mc2-exporter-cli-", dir=generated_root))
    try:
        default = scratch / "default"
        explicit_level_one = scratch / "explicit-level-one"
        level_two_a = scratch / "level-two-a"
        level_two_b = scratch / "level-two-b"

        def run_export(destination: Path, *extra: str) -> None:
            subprocess.run(
                [
                    str(exporter),
                    "--owner-root",
                    str(owner_root),
                    "--output",
                    str(destination),
                    *extra,
                ],
                check=True,
                capture_output=True,
                text=True,
            )

        run_export(default)
        run_export(explicit_level_one, "--level", "1")
        run_export(level_two_a, "--level", "2")
        run_export(level_two_b, "--level", "2")

        assert _tree_bytes(default) == _tree_bytes(explicit_level_one)
        assert _tree_bytes(level_two_a) == _tree_bytes(level_two_b)
        level_two_manifest = json.loads((level_two_a / "manifest.json").read_text())
        assert level_two_manifest["level"] == {
            "external": 2,
            "internal": 1,
            "map_type": {"id": 0, "name": "Day"},
        }
    finally:
        shutil.rmtree(scratch)


if __name__ == "__main__":
    test_selector_mapping_and_rejection()
    test_invalid_cli_has_no_partial_output()
    test_deterministic_level_profiles()
