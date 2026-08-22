#!/usr/bin/env python3
"""Guard grandfathered package-specific exports across all client layers."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import subprocess
import tempfile
from pathlib import Path


HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
BASELINE_PATH = HERE / "compatibility-surface-baseline.json"
SIM_ROOT = REPO / "research/private/remc2-sim-rs"
UNITY_ROOT = REPO / "research/private/remc2-unity-native"
PLUGIN = SIM_ROOT / "target/release/libsim_capi.so"
MANAGED = UNITY_ROOT / "Builds/Linux/MagicCarpetNative_Data/Managed/MagicCarpet.Native.Runtime.dll"
UNITY_MONO = Path(
    "/home/oliver/Unity/Hub/Editor/6000.3.20f1/Editor/Data/MonoBleedingEdge/bin-linux64/mono"
)
UNITY_MCS = UNITY_MONO.with_name("mcs")
AST_GREP = Path("/home/oliver/.cargo/bin/ast-grep")

PACKAGE_TOKEN = re.compile(
    r"(?ix)(?:"
    r"level[_-]?[0-9]+|fireball|nyphur|goat|castle|archer|trader|villager|"
    r"mana[_-]?sphere|spell[_-]?jar|action[_-]?51"
    r")"
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def run(
    command: list[str], cwd: Path = REPO, allowed_statuses: tuple[int, ...] = (0,)
) -> str:
    result = subprocess.run(command, cwd=cwd, text=True, capture_output=True, timeout=300)
    require(
        result.returncode in allowed_statuses,
        f"surface command failed ({' '.join(command)}):\n{result.stdout}{result.stderr}",
    )
    return result.stdout


def digest(values: list[str]) -> str:
    return hashlib.sha256("\n".join(sorted(values)).encode()).hexdigest()


def native_surface(plugin: Path = PLUGIN) -> list[str]:
    require(plugin.is_file(), f"missing candidate plugin: {plugin}")
    output = run(["nm", "-D", "--defined-only", str(plugin)])
    return sorted({
        line.split()[-1]
        for line in output.splitlines()
        if line.split() and line.split()[-1].startswith("mc_")
    })


RUST_PATTERNS = {
    "fn": "pub fn $N($$$A) { $$$B }",
    "async_fn": "pub async fn $N($$$A) { $$$B }",
    "const_fn": "pub const fn $N($$$A) { $$$B }",
    "struct": "pub struct $N { $$$F }",
    "tuple_struct": "pub struct $N($$$F);",
    "enum": "pub enum $N { $$$V }",
    "trait": "pub trait $N { $$$B }",
    "type": "pub type $N = $T;",
    "const": "pub const $N: $T = $V;",
    "static": "pub static $N: $T = $V;",
    "module": "pub mod $N;",
}


def rust_surface() -> list[str]:
    values: set[str] = set()
    source = SIM_ROOT / "crates"
    for kind, pattern in RUST_PATTERNS.items():
        output = run([
            str(AST_GREP), "run", "--lang", "rust", "--pattern", pattern,
            "--json=stream", str(source),
        ], allowed_statuses=(0, 1))
        for line in output.splitlines():
            item = json.loads(line)
            text = item["text"].lstrip()
            if not text.startswith("pub "):
                continue
            name = item.get("metaVariables", {}).get("single", {}).get("N", {}).get("text")
            if not name:
                continue
            path = Path(item["file"]).resolve().relative_to(REPO)
            signature = re.sub(r"\s+", " ", text.split("{", 1)[0].strip())
            values.add(f"{path}:{kind}:{name}:{signature}")
    return sorted(values)


def managed_surface() -> list[str]:
    require(MANAGED.is_file(), f"missing owner-local built managed assembly: {MANAGED}")
    require(UNITY_MCS.is_file() and UNITY_MONO.is_file(), "Unity Mono toolchain missing")
    with tempfile.TemporaryDirectory(prefix="mc-managed-surface-") as directory:
        executable = Path(directory) / "managed-surface-probe.exe"
        run([str(UNITY_MCS), f"-out:{executable}", str(HERE / "managed_surface_probe.cs")])
        output = run([str(UNITY_MONO), str(executable), str(MANAGED)])
    values = []
    for line in output.splitlines():
        kind, owner, signature, access = line.split("\t")
        owner_text = base64.b64decode(owner).decode()
        signature_text = base64.b64decode(signature).decode()
        values.append(f"{access}:{kind}:{owner_text}:{signature_text}")
    return sorted(set(values))


def typescript_surface() -> list[str]:
    patterns = {
        "function": "export function $N($$$A) { $$$B }",
        "class": "export class $N { $$$B }",
        "interface": "export interface $N { $$$B }",
        "type": "export type $N = $T",
        "const": "export const $N = $V",
        "enum": "export enum $N { $$$B }",
    }
    values: set[str] = set()
    source = SIM_ROOT / "web/src"
    for kind, pattern in patterns.items():
        output = run([
            str(AST_GREP), "run", "--lang", "typescript", "--pattern", pattern,
            "--json=stream", str(source),
        ], allowed_statuses=(0, 1))
        for line in output.splitlines():
            item = json.loads(line)
            name = item.get("metaVariables", {}).get("single", {}).get("N", {}).get("text")
            if not name:
                continue
            path = Path(item["file"]).resolve().relative_to(source)
            values.add(f"{path}:{kind}:{name}")
    return sorted(values)


def observe(plugin: Path = PLUGIN) -> dict:
    layers = {
        "native": native_surface(plugin),
        "rust": rust_surface(),
        "managed": managed_surface(),
        "typescript": typescript_surface(),
    }
    return {
        layer: {
            "observed_count": len(values),
            "observed_sha256": digest(values),
            "owner": "mc2-owned-compatibility",
            "disposition": "adapt",
            "compatibility_namespace": f"legacy-{layer}",
            "package_specific_symbols": [
                value for value in values if PACKAGE_TOKEN.search(value)
            ],
        }
        for layer, values in layers.items()
    }


def candidate_baseline(plugin: Path = PLUGIN) -> dict:
    return {
        "schema": "mc2-compatibility-public-surface/v1",
        "purpose": "grandfather package-specific exports while neutral engine APIs are added",
        "policy": {
            "package_specific_match": PACKAGE_TOKEN.pattern,
            "allowed_dispositions": ["keep", "adapt", "retire"],
            "neutral_additions": "allowed",
            "package_specific_additions_or_removals": "rejected until manifest review",
        },
        "layers": observe(plugin),
    }


def package_symbols(layer: dict) -> list[str]:
    require(layer.get("owner"), "surface layer missing owner")
    require(layer.get("disposition") in {"keep", "adapt", "retire"}, "bad disposition")
    require(layer.get("compatibility_namespace"), "missing compatibility namespace")
    symbols = layer.get("package_specific_symbols", [])
    require(len(symbols) == len(set(symbols)), "duplicate surface manifest entry")
    return sorted(symbols)


def verify(baseline: dict, observed: dict) -> None:
    require(baseline.get("schema") == "mc2-compatibility-public-surface/v1", "surface schema")
    for layer in ("native", "rust", "managed", "typescript"):
        expected = package_symbols(baseline["layers"][layer])
        actual = package_symbols(observed[layer])
        missing = sorted(set(expected) - set(actual))
        added = sorted(set(actual) - set(expected))
        require(not missing and not added, f"{layer} package surface drift missing={missing} added={added}")


def negative_controls(baseline: dict, observed: dict) -> None:
    for layer in ("native", "rust", "managed", "typescript"):
        planted = json.loads(json.dumps(observed))
        layer_map = planted[layer]
        symbol = {
            "native": "mc_native_level99_planted",
            "rust": "crates/sim-core/src/lib.rs:fn:level99_planted:pub fn level99_planted()",
            "managed": "public:method:MagicCarpet.Native.Level99Planted:Void Run()",
            "typescript": "game.ts:FunctionDeclaration:level99Planted",
        }[layer]
        layer_map["package_specific_symbols"].append(symbol)
        try:
            verify(baseline, planted)
        except RuntimeError:
            pass
        else:
            raise RuntimeError(f"{layer} planted package symbol was accepted")

    missing = json.loads(json.dumps(baseline))
    first_layer = next(
        layer for layer in missing["layers"].values()
        if layer["package_specific_symbols"]
    )
    first_layer["package_specific_symbols"].pop()
    try:
        verify(missing, observed)
    except RuntimeError:
        pass
    else:
        raise RuntimeError("missing manifest entry was accepted")

    neutral = json.loads(json.dumps(observed))
    neutral["native"]["observed_count"] += 1
    neutral["native"]["observed_sha256"] = "planted-neutral-not-gated"
    verify(baseline, neutral)


def verify_current_surface(plugin: Path = PLUGIN) -> dict[str, int]:
    baseline = json.loads(BASELINE_PATH.read_text(encoding="utf-8"))
    observed = observe(plugin)
    verify(baseline, observed)
    negative_controls(baseline, observed)
    return {
        layer: len(package_symbols(values))
        for layer, values in observed.items()
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--print-candidate", action="store_true")
    parser.add_argument("--plugin", type=Path, default=PLUGIN)
    args = parser.parse_args()
    if args.print_candidate:
        print(json.dumps(candidate_baseline(args.plugin), indent=2, sort_keys=True))
        return
    counts = verify_current_surface(args.plugin)
    print(
        "MC_COMPATIBILITY_SURFACE PASS "
        f"layers=4 package_symbols={json.dumps(counts, sort_keys=True, separators=(',', ':'))} "
        "planted_negatives=5 neutral_addition=allowed"
    )


if __name__ == "__main__":
    main()
