"""Extract exact mutable Goat TMAP states through remc2's own decoder.

This GDB script is private owner-data preservation tooling. It stops after
selectors 152-154 have been initialized, calls the compiled animation routine,
and records only the three small mutable indexed-pixel buffers.
"""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import stat

import gdb


TARGETS = {152, 153, 154}
EXPECTED_EXTENTS = {152: (22, 22), 153: (32, 22), 154: (22, 22)}
EXPECTED_PACKAGE = "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8"
EXPECTED_CLOSURE = "1c3774ca5c032c1a7b8b1c48870840eee33011a2830d08c51f3104fb5c5bcc12"
EXPECTED_CATALOG = "ea9a735c598f2cbed81541366e1fdf8a5fcbc8cef7aecd086bb8a6a9e7c2429e"
EXPECTED_UPSTREAM_COMMIT = "6806c7204d17220ecc0430759f784aea8ee2ddb2"
EXPECTED_UPSTREAM_DIFF = "73fcffd66baaeeeab443d65b1e7bc333eba1f7e106e7918a552c5ac10ad33258"
EXPECTED_COMPILED_SOURCE = (
    "7624dc3d8287f2ce308d06431941d0f25cb27f384820dfe58df4f0b66f9168c2"
)
descriptors: dict[int, int] = {}
descriptor_order: list[int] = []
loader_armed = False


def integer(expression: str) -> int:
    return int(gdb.parse_and_eval(expression))


class InitReturn(gdb.FinishBreakpoint):
    def __init__(self, frame: gdb.Frame, selector: int):
        self.selector = selector
        super().__init__(frame, internal=True)

    def stop(self) -> bool:
        address = int(self.return_value)
        if address == 0:
            raise gdb.GdbError(
                f"selector {self.selector} returned a null animation descriptor"
            )
        descriptors[self.selector] = address
        descriptor_order.append(self.selector)
        return set(descriptors) == TARGETS


class InitEntry(gdb.Breakpoint):
    def stop(self) -> bool:
        names = []
        frame = gdb.newest_frame().older()
        while frame is not None and len(names) < 8:
            names.append(frame.name() or "<unknown>")
            frame = frame.older()
        if "InitTmaps(unsigned short)" not in names and "InitTmaps" not in names:
            raise gdb.GdbError("Goat descriptor initialized outside the armed loader")
        selector = integer("(unsigned short)a3")
        if selector in TARGETS and selector not in descriptors:
            InitReturn(gdb.newest_frame(), selector)
        return False


class LoaderEntry(gdb.Breakpoint):
    def stop(self) -> bool:
        global loader_armed
        selector = integer("(unsigned short)a1")
        if selector != 152:
            return False
        if loader_armed:
            raise gdb.GdbError("Goat TMAP loader epoch repeated")
        loader_armed = True
        InitEntry("sub_721C0_initTmap", internal=True)
        self.enabled = False
        return False


def descriptor(address: int) -> gdb.Value:
    return gdb.parse_and_eval(f"*(type_animations1 *){address:#x}")


def capture(selector: int, address: int, state: str, output: Path) -> dict:
    value = descriptor(address)
    particle = value["Particles_4"]
    width = int(value["Width_18"])
    height = int(value["Height_20"])
    if (width, height) != EXPECTED_EXTENTS[selector]:
        raise gdb.GdbError(
            f"selector {selector} extent {(width, height)} is not qualified"
        )
    count = width * height
    pixels = bytes(
        gdb.selected_inferior().read_memory(int(particle) + 6, count)
    )
    filename = f"{selector}.{state}.indices.u8"
    path = output / filename
    path.write_bytes(pixels)
    path.chmod(0o600)
    return {
        "selector": selector,
        "state": state,
        "file": filename,
        "width": width,
        "height": height,
        "bytes": count,
        "sha256": hashlib.sha256(pixels).hexdigest(),
        "active": int(value["dword_0"]),
        "stream_cursor": int(value["dword_8"]),
        "stream_start": int(value["word_14"]),
        "frame_count": int(value["CountOfFrames_16"]),
        "frame_index": int(value["FrameIndex_22"]),
        "descriptor_slot": int(value["word_24"]),
        "descriptor_selector": int(value["word_26"]),
        "tmap_flags": int(particle.dereference()["word_0"]),
    }


def main() -> None:
    raw_output = os.environ.get("MC2_GOAT_TMAP_ORACLE_OUTPUT")
    if not raw_output:
        raise gdb.GdbError("MC2_GOAT_TMAP_ORACLE_OUTPUT is required")
    output = Path(raw_output).resolve()
    staging = output.with_name(output.name + ".staging")
    if output.exists() or staging.exists():
        raise gdb.GdbError(
            f"oracle output or staging directory already exists: {output}"
        )
    staging.mkdir(mode=0o700, parents=False)

    try:
        gdb.execute("set pagination off")
        gdb.execute("set confirm off")
        gdb.execute("set debuginfod enabled off")
        gdb.execute("set breakpoint pending on")
        LoaderEntry("InitTmaps", internal=True)
        gdb.execute("run")

        if set(descriptors) != TARGETS or descriptor_order != [152, 153, 154]:
            raise gdb.GdbError(
                "Goat descriptors were not initialized as one ordered group: "
                f"{descriptor_order}"
            )
        if integer("bigSprites") != 0:
            raise gdb.GdbError("oracle must use original delta sprites")
        if integer("D41A0_0.terrain_2FECE.MapType") != 1:
            raise gdb.GdbError("oracle did not stop in the Night Level 1 bank")

        rows: list[dict] = []
        for selector in sorted(TARGETS):
            address = descriptors[selector]
            base = capture(selector, address, "reset-base", staging)
            if (
                base["active"] != 1
                or base["frame_count"] != 4
                or base["frame_index"] != 1
                or base["stream_cursor"] != base["stream_start"]
                or base["descriptor_selector"] != selector
            ):
                raise gdb.GdbError(
                    f"selector {selector} descriptor is not at canonical reset"
                )
            rows.append(base)
            for frame_number in range(1, 5):
                gdb.execute(
                    f"call (void)sub_72350((type_animations1 *){address:#x})",
                    to_string=True,
                )
                rows.append(
                    capture(
                        selector,
                        address,
                        f"frame-{frame_number:02d}",
                        staging,
                    )
                )
            gdb.execute(
                f"call (void)sub_72350((type_animations1 *){address:#x})",
                to_string=True,
            )
            closure = capture(
                selector,
                address,
                "closure-frame-01",
                staging,
            )
            first = next(
                row
                for row in rows
                if row["selector"] == selector and row["state"] == "frame-01"
            )
            if (
                closure["sha256"] != first["sha256"]
                or closure["stream_cursor"] != first["stream_cursor"]
                or closure["frame_index"] != first["frame_index"]
                or closure["frame_index"] != 2
            ):
                raise gdb.GdbError(
                    f"selector {selector} failed frame-one cycle closure"
                )
            rows.append(closure)

        binary = Path(os.environ["MC2_TRACE_BINARY"]).resolve(strict=True)
        build_manifest = Path(
            os.environ["MC2_TRACE_BUILD_MANIFEST"]
        ).resolve(strict=True)
        config = Path(os.environ["MC2_TRACE_CONFIG"]).resolve(strict=True)
        owner_root = Path(os.environ["MC2_TRACE_OWNER_ROOT"]).resolve(strict=True)
        repo_root = Path(os.environ["MC2_TRACE_REPO_ROOT"]).resolve(strict=True)
        current_binary = Path(gdb.current_progspace().filename).resolve(strict=True)
        build = json.loads(build_manifest.read_text(encoding="utf-8"))
        binary_sha256 = hashlib.sha256(binary.read_bytes()).hexdigest()
        if current_binary != binary:
            raise gdb.GdbError("GDB executable differs from the bound binary")
        if (
            build.get("schema") != "mc2-frame-trace-build/v1"
            or build.get("target") != "remc2-frame-trace"
            or build.get("build_type") != "Debug"
            or build.get("binary_sha256") != binary_sha256
            or build.get("compiled_source_sha256") != EXPECTED_COMPILED_SOURCE
            or build.get("upstream_commit") != EXPECTED_UPSTREAM_COMMIT
            or build.get("upstream_diff_sha256") != EXPECTED_UPSTREAM_DIFF
        ):
            raise gdb.GdbError("trace build manifest is not the qualified build")

        package = json.loads(
            (repo_root / "research/private/generated/level1-export-m/manifest.json")
            .read_text(encoding="utf-8")
        )
        closure_path = (
            repo_root / "research/private/generated/level1-asset-closure.json"
        )
        closure = json.loads(closure_path.read_text(encoding="utf-8"))
        catalog_path = (
            repo_root
            / "research/private/remc2-atlas/semantics/generated/"
            "owner-asset-catalog.json"
        )
        catalog = json.loads(catalog_path.read_text(encoding="utf-8"))
        if (
            package.get("package_sha256") != EXPECTED_PACKAGE
            or closure.get("closure_sha256") != EXPECTED_CLOSURE
            or catalog.get("catalog_sha256") != EXPECTED_CATALOG
        ):
            raise gdb.GdbError("owner package, closure, or catalog identity changed")

        manifest = {
            "schema": "mc2-private-goat-tmap-oracle/v1",
            "status": "PASS",
            "decoder": "compiled-remc2-sub_72350",
            "big_sprites": False,
            "loader_epoch": {
                "entry": "InitTmaps(152)",
                "descriptor_order": descriptor_order,
                "map_type": 1,
            },
            "bindings": {
                "target": "remc2-frame-trace",
                "binary_sha256": binary_sha256,
                "build_manifest_sha256": hashlib.sha256(
                    build_manifest.read_bytes()
                ).hexdigest(),
                "compiled_source_sha256": build["compiled_source_sha256"],
                "upstream_commit": build["upstream_commit"],
                "upstream_diff_sha256": build["upstream_diff_sha256"],
                "oracle_sha256": hashlib.sha256(
                    Path(__file__).resolve(strict=True).read_bytes()
                ).hexdigest(),
                "gdb_version": gdb.VERSION,
                "config_sha256": hashlib.sha256(config.read_bytes()).hexdigest(),
                "package_identity_sha256": EXPECTED_PACKAGE,
                "closure_identity_sha256": EXPECTED_CLOSURE,
                "closure_file_sha256": hashlib.sha256(
                    closure_path.read_bytes()
                ).hexdigest(),
                "catalog_identity_sha256": EXPECTED_CATALOG,
                "catalog_file_sha256": hashlib.sha256(
                    catalog_path.read_bytes()
                ).hexdigest(),
                "owner_tmaps_dat_sha256": hashlib.sha256(
                    (owner_root / "CD_Files/DATA/TMAPS1-0.DAT").read_bytes()
                ).hexdigest(),
                "owner_tmaps_tab_sha256": hashlib.sha256(
                    (owner_root / "CD_Files/DATA/TMAPS1-0.TAB").read_bytes()
                ).hexdigest(),
            },
            "states": rows,
        }
        manifest_path = staging / "manifest.json"
        manifest_path.write_text(
            json.dumps(manifest, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        manifest_path.chmod(0o600)
        for path in staging.iterdir():
            if stat.S_IMODE(path.stat().st_mode) != 0o600:
                raise gdb.GdbError(f"oracle artifact mode is not 0600: {path.name}")
        if stat.S_IMODE(staging.stat().st_mode) != 0o700:
            raise gdb.GdbError("oracle staging directory mode is not 0700")
        staging.rename(output)
        print(
            "MC_GOAT_TMAP_ORACLE PASS "
            f"selectors={len(TARGETS)} states={len(rows)} output={output}"
        )
        gdb.execute("quit")
    except BaseException:
        if staging.exists():
            shutil.rmtree(staging)
        raise


main()
