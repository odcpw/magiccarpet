#!/usr/bin/env python3
"""Fast Exact-mode protection guard for the private presentation lab.

The guard compares protected Exact surfaces with a pinned committed tree. It
intentionally does not require a clean worktree: concurrent work is reported
as drift instead of being overwritten or silently absorbed into the baseline.

The C header is handled semantically. Existing ABI v2 function declarations
and their signatures must remain byte-for-byte equivalent after whitespace
normalization. New declarations are reported as additive symbols, while any
other header change is forbidden ABI drift. Additive APIs must not mutate the
existing Rust/C#/Unity Exact paths.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, Iterable


DEFAULT_MANIFEST = Path(__file__).with_name("exact-baseline-manifest.json")
SYMBOL_RE = re.compile(r"\b(mc_[A-Za-z0-9_]+)\s*\(")
DECL_RE_TEMPLATE = r"(?ms)(?:^|\n)[^;\n]*\b{name}\s*\([^;]*\);"


def _git(root: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        ["git", "-C", str(root), *args],
        check=check,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def _git_text(root: Path, *args: str, check: bool = True) -> str:
    return _git(root, *args, check=check).stdout.decode("utf-8", "replace").strip()


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _normalise_header(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def _symbols(text: str) -> set[str]:
    return set(SYMBOL_RE.findall(text))


def _symbol_signature(text: str, symbol: str) -> str | None:
    match = re.search(DECL_RE_TEMPLATE.format(name=re.escape(symbol)), text)
    if match is None:
        return None
    return _normalise_header(match.group(0))


def _without_declarations(text: str, symbols: Iterable[str]) -> str:
    result = text
    for symbol in sorted(symbols):
        result = re.sub(DECL_RE_TEMPLATE.format(name=re.escape(symbol)), "", result)
    return _normalise_header(result)


def _load_manifest(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        manifest = json.load(handle)
    if manifest.get("schema_version") != 1:
        raise ValueError(f"unsupported manifest schema: {manifest.get('schema_version')!r}")
    if not manifest.get("baseline", {}).get("source_commit"):
        raise ValueError("manifest baseline.source_commit is required")
    return manifest


def _baseline_blob(root: Path, commit: str, relative: str) -> bytes:
    result = _git(root, "show", f"{commit}:{relative}")
    return result.stdout


def _failure(checks: list[dict[str, Any]], category: str, message: str, **extra: Any) -> None:
    record = {"category": category, "status": "fail", "message": message}
    record.update(extra)
    checks.append(record)


def verify(root: Path, manifest_path: Path) -> dict[str, Any]:
    root = root.resolve()
    manifest = _load_manifest(manifest_path)
    baseline = manifest["baseline"]
    baseline_ref = str(baseline["source_commit"])
    baseline_commit = _git_text(root, "rev-parse", baseline_ref)
    head_commit = _git_text(root, "rev-parse", "HEAD")
    checks: list[dict[str, Any]] = []

    ancestry = _git(root, "merge-base", "--is-ancestor", baseline_commit, head_commit, check=False)
    if ancestry.returncode != 0:
        _failure(
            checks,
            "baseline",
            "pinned Exact baseline is not an ancestor of the current checkout",
            baseline_commit=baseline_commit,
            head_commit=head_commit,
        )
    else:
        checks.append(
            {
                "category": "baseline",
                "status": "pass",
                "message": "pinned Exact baseline is an ancestor",
                "baseline_commit": baseline_commit,
                "head_commit": head_commit,
            }
        )

    for submodule, expected in baseline.get("submodules", {}).items():
        path = root / submodule
        expected_gitlink = _git_text(root, "ls-tree", baseline_commit, "--", submodule).split()
        current_gitlink = _git_text(root, "ls-tree", "HEAD", "--", submodule).split()
        gitlink = expected_gitlink[2] if len(expected_gitlink) >= 3 else "missing"
        current_link = current_gitlink[2] if len(current_gitlink) >= 3 else "missing"
        if gitlink != expected or current_link != expected:
            _failure(
                checks,
                "submodule",
                f"{submodule} gitlink is not at the pinned source commit",
                path=submodule,
                expected=expected,
                actual=current_link,
            )
        elif not (path / ".git").exists():
            checks.append(
                {
                    "category": "submodule",
                    "status": "pass",
                    "message": f"{submodule} gitlink is pinned; checkout is not initialized",
                    "path": submodule,
                    "commit": expected,
                }
            )
        else:
            actual = _git_text(path, "rev-parse", "HEAD", check=False)
            if actual != expected:
                _failure(
                    checks,
                    "submodule",
                    f"{submodule} checkout is not at the pinned source commit",
                    path=submodule,
                    expected=expected,
                    actual=actual,
                )
                continue
            checks.append(
                {
                    "category": "submodule",
                    "status": "pass",
                    "message": f"{submodule} source commit is pinned",
                    "path": submodule,
                    "commit": actual,
                }
            )

    protected = manifest.get("protected_files", {})
    for category, paths in protected.items():
        for relative in paths:
            current_path = root / relative
            try:
                expected_bytes = _baseline_blob(root, baseline_commit, relative)
            except subprocess.CalledProcessError:
                _failure(checks, category, "protected path is absent from the pinned tree", path=relative)
                continue
            if not current_path.is_file():
                _failure(checks, category, "protected path is missing from the checkout", path=relative)
                continue
            actual_bytes = current_path.read_bytes()
            if actual_bytes != expected_bytes:
                _failure(
                    checks,
                    category,
                    "protected Exact file drift",
                    path=relative,
                    expected_sha256=_sha256(expected_bytes),
                    actual_sha256=_sha256(actual_bytes),
                )
            else:
                checks.append(
                    {
                        "category": category,
                        "status": "pass",
                        "message": "protected Exact file matches the pinned tree",
                        "path": relative,
                        "sha256": _sha256(actual_bytes),
                    }
                )

    header_relative = str(manifest["abi_v2_header"])
    try:
        expected_header = _baseline_blob(root, baseline_commit, header_relative).decode("utf-8")
        current_header = (root / header_relative).read_text(encoding="utf-8")
    except (subprocess.CalledProcessError, FileNotFoundError, UnicodeDecodeError) as error:
        _failure(checks, "abi_v2", f"cannot read ABI v2 header: {error}", path=header_relative)
    else:
        expected_symbols = _symbols(expected_header)
        current_symbols = _symbols(current_header)
        removed = sorted(expected_symbols - current_symbols)
        added = sorted(current_symbols - expected_symbols)
        changed = sorted(
            symbol
            for symbol in expected_symbols & current_symbols
            if _symbol_signature(expected_header, symbol) != _symbol_signature(current_header, symbol)
        )
        header_without_additions = _without_declarations(current_header, added)
        expected_normalized = _normalise_header(expected_header)
        if removed:
            _failure(checks, "abi_v2", "existing ABI v2 symbols were removed", path=header_relative, symbols=removed)
        if changed:
            _failure(
                checks,
                "abi_v2",
                "existing ABI v2 symbol signatures changed",
                path=header_relative,
                symbols=changed,
            )
        if header_without_additions != expected_normalized:
            _failure(
                checks,
                "abi_v2",
                "ABI v2 header changed beyond additive function declarations",
                path=header_relative,
                expected_sha256=_sha256(expected_header.encode()),
                actual_sha256=_sha256(current_header.encode()),
            )
        else:
            checks.append(
                {
                    "category": "abi_v2",
                    "status": "pass",
                    "message": "ABI v2 header is stable; additive declarations are isolated",
                    "path": header_relative,
                    "added_symbols": added,
                    "symbol_count": len(current_symbols),
                }
            )

    for assertion in manifest.get("contract_assertions", []):
        relative = str(assertion["path"])
        pattern = str(assertion["regex"])
        path = root / relative
        if not path.is_file():
            _failure(checks, "contract", "contract assertion path is missing", id=assertion["id"], path=relative)
            continue
        content = path.read_text(encoding="utf-8")
        if re.search(pattern, content) is None:
            _failure(
                checks,
                "contract",
                "Exact contract assertion failed",
                id=assertion["id"],
                path=relative,
                regex=pattern,
            )
        else:
            checks.append(
                {
                    "category": "contract",
                    "status": "pass",
                    "message": "Exact contract assertion holds",
                    "id": assertion["id"],
                    "path": relative,
                }
            )

    failures = [check for check in checks if check["status"] == "fail"]
    return {
        "pass": not failures,
        "manifest": str(manifest_path),
        "baseline_commit": baseline_commit,
        "head_commit": head_commit,
        "artifact_identity": {
            "unity_version": baseline.get("unity_version"),
            "native_abi_version": baseline.get("native_abi_version"),
            "artifacts": baseline.get("artifact_identities", {}),
            "exact_profile": baseline.get("exact_profile"),
        },
        "checks": checks,
        "failures": failures,
    }


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_MANIFEST.parents[4], help="repository root")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST, help="baseline manifest")
    parser.add_argument("--json", action="store_true", help="emit machine-readable JSON")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        result = verify(args.root, args.manifest)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        result = {"pass": False, "error": str(error)}
    if args.json:
        print(json.dumps(result, indent=2, sort_keys=True))
    elif result.get("pass"):
        additions = [
            symbol
            for check in result["checks"]
            if check["category"] == "abi_v2" and check["status"] == "pass"
            for symbol in check.get("added_symbols", [])
        ]
        suffix = f"; additive ABI symbols={','.join(additions)}" if additions else ""
        print(f"EXACT BASELINE PASS: {result['baseline_commit']} is protected{suffix}")
    else:
        print("EXACT BASELINE FAIL")
        if "error" in result:
            print(f"  error: {result['error']}")
        for failure in result.get("failures", []):
            location = failure.get("path") or failure.get("id") or failure.get("category")
            print(f"  [{failure['category']}] {location}: {failure['message']}")
    return 0 if result.get("pass") else 1


if __name__ == "__main__":
    sys.exit(main())
