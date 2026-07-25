"""GDB Python observer for the first presented Level 1 gameplay frame."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import sys
import time
import uuid

import gdb

TRACE_ROOT = Path(os.environ["MC2_TRACE_ROOT"]).resolve(strict=True)
sys.path.insert(0, str(TRACE_ROOT))
from trace_format import build_trace, qualify_trace  # noqa: E402

REPO_ROOT = TRACE_ROOT.parents[2]
GAME_RENDER_HD_SOURCE = (
    REPO_ROOT
    / "upstream/magic-carpet-2-hd/remc2/engine/GameRenderHD.cpp"
).resolve(strict=True)
BASIC_SOURCE = (
    REPO_ROOT / "upstream/magic-carpet-2-hd/remc2/engine/Basic.cpp"
).resolve(strict=True)
GAME_BITMAP_SOURCE = (
    REPO_ROOT / "upstream/magic-carpet-2-hd/remc2/engine/GameBitmap.cpp"
).resolve(strict=True)
GAME_UI_SOURCE = (
    REPO_ROOT / "upstream/magic-carpet-2-hd/remc2/engine/GameUI.cpp"
).resolve(strict=True)
PLAYER_INPUT_SOURCE = (
    REPO_ROOT / "upstream/magic-carpet-2-hd/remc2/engine/PlayerInput.cpp"
).resolve(strict=True)
SDL_SOUND_SOURCE = (
    REPO_ROOT
    / "upstream/magic-carpet-2-hd/remc2/portability/port_sdl_sound.cpp"
).resolve(strict=True)
PACKAGE_SHA256 = "730d2fa084265df839f3f2b55ed766ac167a8b152ca4631b5ded9d0bb9104ac8"
PACKAGE_COMPILED_SOURCE_SHA256 = (
    "84993db07f7a8fe2046927f39a56766ad6d1fac208e0bcb554de990fdca5c8ae"
)
CLOSURE_SHA256 = "04e8b237b166161b57d4b86fc5c50a5f96ea32e4ec9b20404ac1d57e4615e751"
CATALOG_SHA256 = "ea9a735c598f2cbed81541366e1fdf8a5fcbc8cef7aecd086bb8a6a9e7c2429e"
MAX_DRAW_CANDIDATES = 64
MAX_SKIPPED_PRESENTS = 256


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def value(expression: str) -> gdb.Value:
    return gdb.parse_and_eval(expression)


def integer(expression: str) -> int:
    return int(value(expression))


class TraceObserver:
    def __init__(self) -> None:
        self.output = Path(os.environ["MC2_TRACE_OUTPUT"]).resolve()
        self.build_manifest_path = Path(
            os.environ["MC2_TRACE_BUILD_MANIFEST"]
        ).resolve(strict=True)
        self.config_path = Path(os.environ["MC2_TRACE_CONFIG"]).resolve(strict=True)
        self.binary_path = Path(os.environ["MC2_TRACE_BINARY"]).resolve(strict=True)
        self.owner_root = Path(os.environ["MC2_TRACE_OWNER_ROOT"]).resolve(strict=True)
        self.started_ns = time.time_ns()
        self.run_uuid = str(uuid.uuid4())
        self.events: list[dict] = []
        self.candidate_events: list[dict] = []
        self.header: dict | None = None
        self.armed = False
        self.closed = False
        self.candidate_index = 0
        self.candidate_final_draws = 0
        self.candidate_tmap_count = 0
        self.skipped_presents: list[dict] = []
        self.prequalified_candidates: list[dict] = []
        self.transcript: list[dict] = []
        self.active_present: dict | None = None
        self.qualified_epoch: int | None = None
        self.vga_entries = 0
        self.subblit_entries = 0
        self.failure: str | None = None
        self.bitmap_pointer_index: dict[int, list[tuple[str, int]]] | None = None
        self.bitmap_bank_counts: dict[str, int] = {}
        self.language_pointer_index: dict[int, list[int]] | None = None
        self.candidate_cursor_seen = False
        self.candidate_cursor_state: dict | None = None
        self.cursor_source_state: dict | None = None
        self.hook_resolution_startup: dict | None = None
        self.output.mkdir(mode=0o700, parents=False, exist_ok=False)

    def fail(self, message: str) -> bool:
        self.failure = message
        return True

    def add_transcript(self, kind: str, **details: object) -> None:
        self.transcript.append(
            {
                "seq": len(self.transcript) + 1,
                "epoch": self.candidate_index,
                "kind": kind,
                **details,
            }
        )

    @staticmethod
    def frame_path() -> list[str]:
        names = []
        frame = gdb.newest_frame().older()
        while frame is not None and len(names) < 12:
            names.append(frame.name() or "<unknown>")
            frame = frame.older()
        return names

    @staticmethod
    def caller_class(path: list[str]) -> str:
        if (
            path
            and path[0] == "sub_90478_VGA_Blit320"
            and "BlendAndBlit_40F80" in path
            and "DrawAndEventsInGame_47560" in path
        ):
            return "expected_gameplay_blit"
        if any("Palette" in name or "palette" in name for name in path):
            return "palette_path"
        return "other"

    @staticmethod
    def pointer_class(pointer: int) -> str:
        if pointer == 0:
            return "null"
        if pointer == integer("(uintptr_t)pdwScreenBuffer_351628"):
            return "screen_buffer"
        return "other_non_null"

    def capture_header(self) -> None:
        build = json.loads(self.build_manifest_path.read_text(encoding="utf-8"))
        if build["binary_sha256"] != file_sha256(self.binary_path):
            raise RuntimeError("trace binary differs from build manifest")
        local_slot = integer("D41A0_0.LevelIndex_0xc")
        if not 0 <= local_slot < 8:
            raise RuntimeError(f"local player slot is out of range: {local_slot}")
        player_index = integer(
            f"D41A0_0.array_0x2BDE[{local_slot}].playerIndex_0x00a_2BE4_11240"
        )
        if not 0 <= player_index < 1000:
            raise RuntimeError(f"local player entity is out of range: {player_index}")
        entity = f"Entities_EA3E4[{player_index}]"
        self.header = {
            "scope": "PRIVATE_TRACK_P_OWNER_DATA",
            "bindings": {
                "level_export_package_sha256": PACKAGE_SHA256,
                "level_export_compiled_source_sha256": PACKAGE_COMPILED_SOURCE_SHA256,
                "static_closure_sha256": CLOSURE_SHA256,
                "owner_catalog_sha256": CATALOG_SHA256,
                "upstream_commit": build["upstream_commit"],
                "upstream_diff_sha256": build["upstream_diff_sha256"],
                "trace_compiled_source_sha256": build["compiled_source_sha256"],
                "observer_sha256": file_sha256(Path(__file__).resolve(strict=True)),
                "trace_format_sha256": file_sha256(
                    (TRACE_ROOT / "trace_format.py").resolve(strict=True)
                ),
                "binary_sha256": build["binary_sha256"],
                "build_manifest_sha256": file_sha256(self.build_manifest_path),
                "gdb_version": gdb.VERSION,
                "build_type": build["build_type"],
            },
            "invocation": {
                "argv": [
                    "remc2-frame-trace",
                    "--mode_test_regressions", "1",
                    "--text_output_to_console",
                    "--set_level", "0",
                    "--config_file_path", str(self.config_path),
                    "--memimages_path",
                    str(self.owner_root / "memimages/regressions"),
                    "--set_max_regressions_steps", "0",
                ],
                "config": {
                    "path": str(self.config_path),
                    "sha256": file_sha256(self.config_path),
                },
                "recording": {"mode": "none", "path": None, "sha256": None},
            },
            "camera": {
                "local_player_slot": local_slot,
                "entity_index": player_index,
                "position": {
                    "x": integer(f"{entity}->position_0x4C_76.x"),
                    "y": integer(f"{entity}->position_0x4C_76.y"),
                    "z": integer(f"{entity}->position_0x4C_76.z"),
                },
                "yaw": integer(f"{entity}->yaw_0x1C_28"),
                "pitch": integer(f"{entity}->pitch_0x1E_30"),
                "roll": integer(f"{entity}->roll_0x20_32"),
            },
            "settings": {
                "map_type": integer("D41A0_0.terrain_2FECE.MapType"),
                "terrain_graphics_flags": integer(
                    "D41A0_0.terrain_2FECE.byte_0x2FED2"
                ),
                "vga_resolution_selector": integer(
                    "x_WORD_180660_VGA_type_resolution"
                ),
                "texture_size": integer("x_BYTE_D41B5_texture_size"),
                "screen_width": integer("viewPort.Width_DE564"),
                "screen_height": integer("viewPort.Height_DE568"),
                "sky_enabled": integer(
                    "D41A0_0.m_GameSettings.m_Graphics.m_wSky"
                ),
                "is_cave": bool(integer("isCaveLevel_D41B6")),
                "hq_sound": bool(integer("hqsound")),
                "ogg_music": bool(integer("oggmusic")),
                "render_threads_configured": 0,
                "active_cursor": {
                    "draw_gate": bool(integer("x_DWORD_E3768")),
                    "outer_suppression": bool(integer("x_BYTE_E3766")),
                    "selector": integer("(unsigned char)CURSOR_SPRITE_INDEX_D419E"),
                    "data_pointer_non_null": bool(
                        integer("(uintptr_t)x_DWORD_180730_cursor_data")
                    ),
                    "source_provenance": self.cursor_source_state,
                },
                "audio": {
                    "sound_able": bool(integer("soundAble_E3798")),
                    "sound_active": bool(integer("soundActive_E3799")),
                    "max_loaded_wav_index": integer("MaxLoadedWavIndex_180B50"),
                },
            },
        }

    @staticmethod
    def resolved_hook_proof(
        breakpoint: gdb.Breakpoint,
        expected_source: Path,
        expected_line: int,
        expected_function: str,
    ) -> dict:
        expected_source = expected_source.resolve(strict=True)
        if (
            not breakpoint.is_valid()
            or breakpoint.enabled is not True
            or breakpoint.pending is not False
            or breakpoint.location != f"{expected_source}:{expected_line}"
        ):
            raise RuntimeError(
                f"{expected_function} breakpoint is invalid, disabled, pending, "
                "or bound to the wrong requested location"
            )
        locations = breakpoint.locations
        if len(locations) != 1:
            raise RuntimeError(
                f"{expected_function} resolved to {len(locations)} locations"
            )
        location = locations[0]
        source = location.source
        if (
            location.enabled is not True
            or not isinstance(source, tuple)
            or len(source) != 2
            or Path(source[0]).resolve(strict=True) != expected_source
            or source[1] != expected_line
            or expected_function not in (location.function or "")
        ):
            raise RuntimeError(
                f"{expected_function} breakpoint resolved to the wrong source/function"
            )
        return {
            "breakpoint_valid": True,
            "breakpoint_enabled": True,
            "breakpoint_pending": False,
            "requested_source": expected_source.relative_to(REPO_ROOT).as_posix(),
            "requested_line": expected_line,
            "resolved_location_count": 1,
            "resolved_source": expected_source.relative_to(REPO_ROOT).as_posix(),
            "resolved_line": source[1],
            "resolved_function": location.function,
            "resolved_location_enabled": True,
        }

    def capture_hook_resolution(self) -> dict:
        return {
            "sound": self.resolved_hook_proof(
                SOUND_BREAKPOINT,
                SDL_SOUND_SOURCE,
                594,
                "SOUND_start_sample",
            ),
            "music": self.resolved_hook_proof(
                MUSIC_BREAKPOINT,
                SDL_SOUND_SOURCE,
                104,
                "SOUND_start_sequence",
            ),
        }

    def capture_startup_hook_resolution(self) -> None:
        if self.hook_resolution_startup is not None:
            raise RuntimeError("startup hook resolution was captured more than once")
        self.hook_resolution_startup = self.capture_hook_resolution()

    def finalize_prequalified_candidate(self) -> None:
        if self.candidate_index == 0:
            return
        diagnostics = [
            item["ordinal"]
            for item in self.skipped_presents
            if item["epoch"] == self.candidate_index
        ]
        self.add_transcript(
            "candidate_rejected",
            final_tmap_count=self.candidate_tmap_count,
            diagnostic_ordinals=diagnostics,
            reason="no_expected_gameplay_present_before_next_epoch",
        )
        self.prequalified_candidates.append(
            {
                "epoch": self.candidate_index,
                "final_tmap_count": self.candidate_tmap_count,
                "diagnostic_ordinals": diagnostics,
                "reason": "no_expected_gameplay_present_before_next_epoch",
            }
        )

    def begin_candidate(self) -> None:
        if self.closed:
            return
        if self.active_present is not None:
            raise RuntimeError("candidate epoch changed during an active present")
        if self.armed:
            raise RuntimeError("candidate epoch changed during the qualified chain")
        self.finalize_prequalified_candidate()
        self.candidate_index += 1
        self.candidate_final_draws = 0
        self.candidate_tmap_count = 0
        self.candidate_events = []
        self.candidate_cursor_seen = False
        self.candidate_cursor_state = None
        if self.candidate_index > MAX_DRAW_CANDIDATES:
            raise RuntimeError(
                f"render-bearing present not found within {MAX_DRAW_CANDIDATES} candidates"
            )
        self.add_transcript("candidate_begin")

    def record_tmap(self, selector: int) -> None:
        if self.closed:
            return
        if self.candidate_index == 0:
            raise RuntimeError("final draw occurred outside a gameplay candidate")
        self.candidate_final_draws += 1
        self.candidate_tmap_count += 1
        if self.candidate_tmap_count == 1:
            self.add_transcript("first_final_tmap", selector=selector)
        self.candidate_events.append(
            {
                "category": "tmap",
                "bank": "TMAPS1-0",
                "selector": selector,
                "resolution": "unique",
                "hook": "GameRenderHD::DrawSprite_41BD3",
            }
        )

    def record_asset(self, event: dict) -> None:
        if self.closed:
            return
        if self.candidate_index == 0:
            raise RuntimeError(
                f"{event.get('category', 'asset')} draw occurred outside a gameplay candidate"
            )
        self.candidate_events.append(event)

    @staticmethod
    def resolve_block_selector(texture: int) -> int:
        if texture == 0:
            raise RuntimeError("terrain compositor received a null block pointer")
        texture_size = integer("x_BYTE_D41B5_texture_size")
        if texture_size <= 0 or 256 % texture_size:
            raise RuntimeError(
                f"invalid terrain block texture size: {texture_size}"
            )
        base = integer("(uintptr_t)BLOCK32DAT_BEGIN_BUFFER")
        offset = texture - base
        row_stride = texture_size * 256
        columns = 256 // texture_size
        if (
            offset < 0
            or offset >= texture_size * row_stride
            or offset % texture_size
            or (offset % row_stride) // texture_size >= columns
        ):
            raise RuntimeError(
                f"terrain pointer offset {offset} is outside the block atlas layout"
            )
        selector = (offset // row_stride) * columns
        selector += (offset % row_stride) // texture_size
        expected = integer(
            f"(uintptr_t)x_DWORD_DDF50_texture_adresses._M_elems[{selector}]"
        )
        if expected != texture:
            raise RuntimeError(
                f"terrain selector {selector} table pointer does not match compositor"
            )
        return selector

    def record_terrain(self, hook: str) -> None:
        selector = self.resolve_block_selector(
            integer("(uintptr_t)x_DWORD_DE55C_ActTexture")
        )
        self.record_asset(
            {
                "category": "terrain",
                "bank": "BL32N0-0",
                "selector": selector,
                "resolution": "unique",
                "hook": hook,
                "stage": "post_viewport_cull",
            }
        )

    def record_sky(self) -> None:
        self.record_asset(
            {
                "category": "sky",
                "bank": "SKYN0-0",
                "selector": "full_bank",
                "resolution": "unique",
                "span_offset": 0,
                "span_length": 65536,
                "hook": "GameRenderHD::DrawSky_40950",
                "stage": "active_compositor",
            }
        )

    def load_bitmap_pointer_index(self) -> None:
        if self.bitmap_pointer_index is not None:
            return
        pointer_size = integer("sizeof(void*)")
        record_size = integer("sizeof(bitmap_pos_struct_t)")
        if pointer_size != 8 or record_size != 10:
            raise RuntimeError(
                f"unexpected bitmap ABI: pointer={pointer_size}, record={record_size}"
            )
        banks = {
            0: "POINTERS",
            1: "FONT0",
            2: "FONT1",
            4: "MSPRN0-0",
            6: "HFONT3",
            8: "BUILD0-0",
        }
        index: dict[int, list[tuple[str, int]]] = {}
        inferior = gdb.selected_inferior()
        for bank_index, bank in banks.items():
            count = integer(
                f"(*filearray_2aa18c[{bank_index}].end_buffer)"
                f" - (*filearray_2aa18c[{bank_index}].begin_buffer)"
            )
            base = integer(
                f"(uintptr_t)(*filearray_2aa18c[{bank_index}].posistruct)"
            )
            if count < 0 or count > 4096 or (count and base == 0):
                raise RuntimeError(
                    f"invalid {bank} bitmap index base={base:#x} count={count}"
                )
            self.bitmap_bank_counts[bank] = count
            raw = (
                bytes(inferior.read_memory(base, count * record_size))
                if count
                else b""
            )
            for selector in range(count):
                offset = selector * record_size
                pointer = int.from_bytes(
                    raw[offset : offset + pointer_size], byteorder="little"
                )
                if pointer:
                    index.setdefault(pointer, []).append((bank, selector))
        self.bitmap_pointer_index = index

    def resolve_bitmap_pointer(
        self, pointer: int, allowed_banks: set[str]
    ) -> tuple[str, int]:
        if pointer == 0:
            raise RuntimeError("final bitmap received a null data pointer")
        self.load_bitmap_pointer_index()
        assert self.bitmap_pointer_index is not None
        candidates = [
            candidate
            for candidate in self.bitmap_pointer_index.get(pointer, [])
            if candidate[0] in allowed_banks
        ]
        if len(candidates) != 1:
            raise RuntimeError(
                f"bitmap pointer resolved to {len(candidates)} allowed selectors: "
                f"{candidates}"
            )
        return candidates[0]

    @staticmethod
    def matching_bitmap_records(
        pointer: int,
        width: int,
        height: int,
        base: int,
        count: int,
        bank: str,
    ) -> list[tuple[str, int]]:
        pointer_size = integer("sizeof(void*)")
        record_size = integer("sizeof(bitmap_pos_struct_t)")
        if count <= 0 or count > 4096 or base == 0:
            raise RuntimeError(
                f"invalid {bank} bitmap index base={base:#x} count={count}"
            )
        raw = bytes(
            gdb.selected_inferior().read_memory(base, count * record_size)
        )
        candidates: list[tuple[str, int]] = []
        for selector in range(count):
            offset = selector * record_size
            candidate_pointer = int.from_bytes(
                raw[offset : offset + pointer_size], byteorder="little"
            )
            candidate_width = raw[offset + pointer_size]
            candidate_height = raw[offset + pointer_size + 1]
            if (
                candidate_pointer == pointer
                and candidate_width == width
                and candidate_height == height
            ):
                candidates.append((bank, selector))
        return candidates

    @classmethod
    def resolve_cursor_source(
        cls, pointer: int, width: int, height: int
    ) -> tuple[str, int]:
        if pointer == 0:
            raise RuntimeError("cursor selector received a null data pointer")
        pointer_count = integer(
            "(*filearray_2aa18c[0].end_buffer)"
            " - (*filearray_2aa18c[0].begin_buffer)"
        )
        pointer_base = integer(
            "(uintptr_t)(*filearray_2aa18c[0].posistruct)"
        )
        runtime_base = integer("(uintptr_t)xy_DWORD_17DED4_spritestr")
        candidates = cls.matching_bitmap_records(
            pointer,
            width,
            height,
            pointer_base,
            pointer_count,
            "POINTERS",
        )
        candidates.extend(
            cls.matching_bitmap_records(
                pointer,
                width,
                height,
                runtime_base,
                1000,
                "RUNTIME_SPRITES",
            )
        )
        if len(candidates) != 1:
            raise RuntimeError(
                "cursor source record resolved to "
                f"{len(candidates)} selectors: {candidates}"
            )
        return candidates[0]

    def record_bitmap(self, pointer_expression: str, hook: str) -> None:
        if self.candidate_index == 0 or self.closed:
            return
        bank, selector = self.resolve_bitmap_pointer(
            integer(f"(uintptr_t){pointer_expression}"),
            {"POINTERS", "MSPRN0-0", "BUILD0-0"},
        )
        self.record_asset(
            {
                "category": "bitmap",
                "bank": bank,
                "selector": selector,
                "resolution": "unique",
                "hook": hook,
                "stage": "final_bitmap_draw",
            }
        )

    def record_cursor(self) -> None:
        if self.candidate_index == 0 or self.closed or self.candidate_cursor_seen:
            return
        selector = integer("(unsigned char)CURSOR_SPRITE_INDEX_D419E")
        visible_width = integer("x_DWORD_1806FC_mouse_invx")
        visible_height = integer("x_DWORD_1806FC_mouse_invy")
        source = self.cursor_source_state
        if (
            source is None
            or source.get("resolution") != "unique"
            or source.get("bank") != "POINTERS"
        ):
            raise RuntimeError(
                f"active cursor selector {selector} lacks unique POINTERS provenance: "
                f"{source}"
            )
        self.candidate_cursor_state = {
            "bank": source["bank"],
            "nominal_selector": selector,
            "source_selector": source["selector"],
            "resolution": "unique",
            "visible_width": visible_width,
            "visible_height": visible_height,
            "post_clip_hook_reached": True,
        }
        self.candidate_cursor_seen = True
        if visible_width <= 0 or visible_height <= 0:
            return
        self.record_asset(
            {
                "category": "cursor",
                "bank": source["bank"],
                "selector": source["selector"],
                "nominal_selector": selector,
                "resolution": "unique",
                "hook": "DrawCursor_8C635:post_clip_visible_extent",
                "stage": "active_cursor_draw",
            }
        )

    def record_cursor_source(self) -> None:
        pointer = integer("(uintptr_t)a2.data")
        width = integer("(unsigned char)a2.width_4")
        height = integer("(unsigned char)a2.height_5")
        if pointer == 0:
            self.cursor_source_state = {
                "bank": "POINTERS",
                "selector": None,
                "resolution": "null_cursor",
                "hook": "SetCursor_8CD27:post_size_assignment",
            }
            return
        bank, selector = self.resolve_cursor_source(pointer, width, height)
        self.cursor_source_state = {
            "bank": bank,
            "selector": selector,
            "resolution": "unique",
            "hook": "SetCursor_8CD27:post_size_assignment",
            "caller_path": self.frame_path(),
        }

    @staticmethod
    def read_c_bytes(pointer: int, maximum: int = 4096) -> bytes:
        if pointer == 0:
            raise RuntimeError("text draw received a null string pointer")
        inferior = gdb.selected_inferior()
        collected = bytearray()
        while len(collected) < maximum:
            chunk_size = min(64, maximum - len(collected))
            chunk = bytes(inferior.read_memory(pointer + len(collected), chunk_size))
            terminator = chunk.find(b"\0")
            if terminator >= 0:
                collected.extend(chunk[:terminator])
                return bytes(collected)
            collected.extend(chunk)
        raise RuntimeError(f"text draw exceeded the {maximum}-byte trace bound")

    def load_language_pointer_index(self) -> None:
        if self.language_pointer_index is not None:
            return
        pointer_size = integer("sizeof(void*)")
        base = integer("(uintptr_t)&x_DWORD_E9C4C_langindexbuffer[0]")
        raw = bytes(gdb.selected_inferior().read_memory(base, 1000 * pointer_size))
        index: dict[int, list[int]] = {}
        for selector in range(1000):
            offset = selector * pointer_size
            pointer = int.from_bytes(
                raw[offset : offset + pointer_size], byteorder="little"
            )
            if pointer:
                index.setdefault(pointer, []).append(selector)
        self.language_pointer_index = index

    def record_text(self, pointer_expression: str, role: str, hook: str) -> None:
        if self.candidate_index == 0 or self.closed:
            return
        pointer = integer(f"(uintptr_t){pointer_expression}")
        content = self.read_c_bytes(pointer)
        self.load_language_pointer_index()
        assert self.language_pointer_index is not None
        selectors = self.language_pointer_index.get(pointer, [])
        event = {
            "category": "text",
            "role": role,
            "byte_length": len(content),
            "content_sha256": hashlib.sha256(content).hexdigest(),
            "bytes_hex": content.hex(),
            "hook": hook,
            "stage": "actual_text_draw",
        }
        if selectors:
            if len(selectors) != 1:
                raise RuntimeError(
                    f"language pointer resolves to ambiguous selectors: {selectors}"
                )
            event.update(
                {
                    "bank": "L1.TXT",
                    "selector": selectors[0],
                    "resolution": "unique",
                    "source": "language_index",
                }
            )
        else:
            event["source"] = "raw_runtime_bytes"
        self.record_asset(event)

    def record_glyph(self) -> None:
        if self.candidate_index == 0 or self.closed:
            return
        raw_byte = integer("(unsigned char)*v4")
        selector = raw_byte + 1
        bank = "HFONT3" if integer("help_VGA_type_resolution") == 8 else "FONT1"
        self.load_bitmap_pointer_index()
        if not 0 <= selector < self.bitmap_bank_counts.get(bank, 0):
            raise RuntimeError(
                f"glyph {raw_byte:#x}+1 is outside {bank} selector bounds"
            )
        self.record_asset(
            {
                "category": "glyph",
                "bank": bank,
                "selector": selector,
                "resolution": "unique",
                "source_byte": raw_byte,
                "hook": "sub_6F940_sub_draw_text:final_glyph_branch",
                "stage": "final_glyph_draw",
            }
        )

    def record_sound(self) -> None:
        if self.candidate_index == 0 or self.closed:
            return
        selector = integer("S->id2_19")
        maximum = integer("MaxLoadedWavIndex_180B50")
        if not 0 <= selector <= maximum:
            raise RuntimeError(
                f"sound selector {selector} is outside loaded range 0..{maximum}"
            )
        high_quality = bool(integer("hqsound"))
        self.record_asset(
            {
                "category": "sound",
                "bank": "SOUND.DAT",
                "selector": selector,
                "resolution": "unique",
                "quality": "hq_44khz" if high_quality else "native",
                "sample_buffer": "start_44mhz" if high_quality else "start_2_3[0]",
                "sample_length": integer("S->len_4_5[0]"),
                "playback_rate": integer("S->playback_rate_15"),
                "hook": "SOUND_start_sample:after_unit_test_guard",
                "stage": "actual_sample_start",
            }
        )

    def record_music(self) -> None:
        if self.candidate_index == 0 or self.closed:
            return
        selector = integer("sequence_num")
        if not 0 <= selector <= 20:
            raise RuntimeError(f"music sequence is outside loaded range: {selector}")
        self.record_asset(
            {
                "category": "music",
                "bank": "MUSIC.DAT",
                "selector": selector,
                "resolution": "unique",
                "hook": "SOUND_start_sequence:after_unit_test_guard",
                "stage": "actual_sequence_start",
            }
        )

    def begin_present(self, src_buffer: int) -> bool:
        if self.active_present is not None:
            raise RuntimeError("nested VGA_Blit calls are not supported")
        if self.candidate_index == 0 and not self.armed:
            return False
        pointer_class = self.pointer_class(src_buffer)
        caller_path = self.frame_path()
        caller_class = self.caller_class(caller_path)
        qualifies = (
            self.candidate_tmap_count > 0
            and pointer_class == "screen_buffer"
            and caller_class == "expected_gameplay_blit"
        )
        if qualifies:
            self.armed = True
            self.qualified_epoch = self.candidate_index
            self.events = list(self.candidate_events)
            self.vga_entries += 1
            if self.vga_entries != 1:
                raise RuntimeError(
                    "more than one qualified VGA_Blit entered before close"
                )
            self.add_transcript(
                "qualified_vga_entry",
                pointer_class=pointer_class,
                caller_class=caller_class,
                caller_path=caller_path,
                final_tmap_count=self.candidate_tmap_count,
            )
            self.capture_header()
            self.active_present = {
                "kind": "qualified",
                "epoch": self.candidate_index,
                "subblit_entries": 0,
            }
            return True
        if len(self.skipped_presents) >= MAX_SKIPPED_PRESENTS:
            raise RuntimeError(f"more than {MAX_SKIPPED_PRESENTS} excluded presents")
        diagnostic = {
            "ordinal": len(self.skipped_presents) + 1,
            "epoch": self.candidate_index,
            "source": "VGA_Blit",
            "pointer_class": pointer_class,
            "caller_class": caller_class,
            "caller_path": caller_path,
            "final_tmap_count_at_entry": self.candidate_tmap_count,
            "subblit_entries": 0,
            "return_proven": False,
        }
        self.skipped_presents.append(diagnostic)
        self.add_transcript(
            "excluded_vga_entry",
            diagnostic_ordinal=diagnostic["ordinal"],
            pointer_class=pointer_class,
            caller_class=caller_class,
            final_tmap_count=self.candidate_tmap_count,
        )
        self.active_present = {
            "kind": "excluded",
            "epoch": self.candidate_index,
            "diagnostic_ordinal": diagnostic["ordinal"],
            "subblit_entries": 0,
        }
        return True

    def observe_subblit(self) -> int | None:
        if self.active_present is None:
            if self.candidate_index == 0 and not self.armed:
                return None
            if len(self.skipped_presents) >= MAX_SKIPPED_PRESENTS:
                raise RuntimeError(f"more than {MAX_SKIPPED_PRESENTS} excluded presents")
            caller_path = self.frame_path()
            diagnostic = {
                "ordinal": len(self.skipped_presents) + 1,
                "epoch": self.candidate_index,
                "source": "direct_SubBlit",
                "pointer_class": "not_applicable",
                "caller_class": self.caller_class(caller_path),
                "caller_path": caller_path,
                "final_tmap_count_at_entry": self.candidate_tmap_count,
                "subblit_entries": 1,
                "return_proven": False,
            }
            self.skipped_presents.append(diagnostic)
            self.add_transcript(
                "excluded_direct_subblit_entry",
                diagnostic_ordinal=diagnostic["ordinal"],
                caller_class=diagnostic["caller_class"],
                final_tmap_count=self.candidate_tmap_count,
            )
            return diagnostic["ordinal"]
        if self.active_present["epoch"] != self.candidate_index:
            raise RuntimeError("SubBlit crossed candidate epochs")
        self.active_present["subblit_entries"] += 1
        if self.active_present["subblit_entries"] != 1:
            raise RuntimeError("VGA_Blit entered SubBlit more than once")
        if self.active_present["kind"] == "qualified":
            self.subblit_entries += 1
            self.add_transcript("qualified_subblit", subblit_entries=1)
        else:
            ordinal = self.active_present["diagnostic_ordinal"]
            self.skipped_presents[ordinal - 1]["subblit_entries"] = 1
            self.add_transcript(
                "excluded_vga_subblit",
                diagnostic_ordinal=ordinal,
                subblit_entries=1,
            )
        return None

    def finish_direct_subblit(self, ordinal: int, epoch: int) -> None:
        if self.candidate_index != epoch or self.armed:
            raise RuntimeError("direct SubBlit return crossed candidate epochs")
        diagnostic = self.skipped_presents[ordinal - 1]
        diagnostic["return_proven"] = True
        self.add_transcript(
            "excluded_direct_subblit_return",
            diagnostic_ordinal=ordinal,
            return_proven=True,
        )

    def finish_present(self) -> bool:
        if self.active_present is None:
            raise RuntimeError("VGA_Blit returned without an active present")
        present = self.active_present
        self.active_present = None
        if present["epoch"] != self.candidate_index:
            raise RuntimeError("VGA_Blit return crossed candidate epochs")
        if present["subblit_entries"] != 1:
            raise RuntimeError(
                f"{present['kind']} VGA_Blit reached SubBlit "
                f"{present['subblit_entries']} times"
            )
        if present["kind"] == "excluded":
            ordinal = present["diagnostic_ordinal"]
            diagnostic = self.skipped_presents[ordinal - 1]
            diagnostic["return_proven"] = True
            self.add_transcript(
                "excluded_vga_return",
                diagnostic_ordinal=ordinal,
                return_proven=True,
            )
            return False
        self.add_transcript("qualified_vga_return", return_proven=True)
        self.finish()
        return True

    def resolve_tmap_selector(self) -> int:
        texture = integer("(uintptr_t)str_F2C20ar.dword0x02_data")
        if texture == 0:
            raise RuntimeError("DrawSprite received a null final texture pointer")
        candidates = []
        for index in range(504):
            pointer = integer(f"(uintptr_t)str_DWORD_F66F0x[{index}]")
            if pointer == 0:
                continue
            candidate = integer(
                f"(uintptr_t)str_DWORD_F66F0x[{index}]->textureBuffer"
            )
            if candidate == texture:
                candidates.append(index)
        if len(candidates) != 1:
            raise RuntimeError(
                f"final TMAP pointer resolved to {len(candidates)} selectors: {candidates}"
            )
        return candidates[0]

    def finish(self) -> None:
        if self.header is None:
            raise RuntimeError("frame window closed without a header")
        if self.vga_entries != 1 or self.subblit_entries != 1:
            raise RuntimeError(
                f"expected one present, got VGA={self.vga_entries} SubBlit={self.subblit_entries}"
            )
        if self.qualified_epoch != self.candidate_index:
            raise RuntimeError("qualified frame crossed candidate epochs")
        self.add_transcript("closed", no_second_qualified_present=True)
        if self.hook_resolution_startup is None:
            raise RuntimeError("sound/music hook resolution was not proven at startup")
        hook_resolution_close = self.capture_hook_resolution()
        categories = {event.get("category") for event in self.events}
        empty_category_evidence = {}
        if "cursor" not in categories:
            cursor = self.header["settings"]["active_cursor"]
            if (
                cursor["draw_gate"]
                and not cursor["outer_suppression"]
                and (
                    self.candidate_cursor_state is None
                    or (
                        self.candidate_cursor_state["visible_width"] > 0
                        and self.candidate_cursor_state["visible_height"] > 0
                    )
                )
            ):
                raise RuntimeError(
                    "cursor draw path expected visible pixels but none were observed"
                )
            if cursor["outer_suppression"]:
                cursor_reason = "cursor_outer_suppression_true"
            elif not cursor["draw_gate"]:
                cursor_reason = "cursor_draw_gate_false"
            else:
                cursor_reason = "cursor_post_clip_extent_empty"
            empty_category_evidence["cursor"] = {
                "epoch": self.qualified_epoch,
                "hook": "DrawCursor_8C635:post_clip_visible_extent",
                "hook_armed": True,
                "actual_entry_count": 0,
                "reason": cursor_reason,
                "active_selector_state": cursor["selector"],
                "post_clip_state": self.candidate_cursor_state,
            }
        if "sound" not in categories:
            empty_category_evidence["sound"] = {
                "epoch": self.qualified_epoch,
                "hook": "SOUND_start_sample:after_unit_test_guard",
                "hook_armed": True,
                "actual_entry_count": 0,
                "reason": "no_actual_sample_start_in_qualified_epoch",
                "resolved_hook_proof": {
                    "startup": self.hook_resolution_startup["sound"],
                    "close": hook_resolution_close["sound"],
                },
            }
        if "music" not in categories:
            empty_category_evidence["music"] = {
                "epoch": self.qualified_epoch,
                "hook": "SOUND_start_sequence:after_unit_test_guard",
                "hook_armed": True,
                "actual_entry_count": 0,
                "reason": "no_actual_sequence_start_in_qualified_epoch",
                "resolved_hook_proof": {
                    "startup": self.hook_resolution_startup["music"],
                    "close": hook_resolution_close["music"],
                },
            }
        self.header["window"] = {
            "name": "first_render_bearing_gameplay_present",
            "candidate": "ordered DrawAndEventsInGame_47560 epochs",
            "arm": (
                "earliest epoch with final TMAP then screen-buffer VGA_Blit "
                "through sub_90478_VGA_Blit320 and BlendAndBlit_40F80"
            ),
            "close": "qualified VGA_Blit return after exactly one SubBlit",
            "required_presents": 1,
            "qualified_epoch": self.qualified_epoch,
            "qualified_final_tmap_count": self.candidate_tmap_count,
            "bounds": {
                "max_candidates": MAX_DRAW_CANDIDATES,
                "max_skipped_presents": MAX_SKIPPED_PRESENTS,
            },
            "prequalified_candidates": self.prequalified_candidates,
            "pre_window_diagnostics": self.skipped_presents,
            "lifecycle_transcript": self.transcript,
            "empty_category_evidence": empty_category_evidence,
        }
        trace_path = self.output / "trace.jsonl"
        trace_path.write_text(build_trace(self.header, self.events), encoding="utf-8")
        os.chmod(trace_path, 0o600)
        qualification = qualify_trace(
            trace_path, require_complete_categories=True
        )
        qualification_path = self.output / "qualification.json"
        qualification_path.write_text(
            json.dumps(qualification, sort_keys=True, indent=2) + "\n",
            encoding="utf-8",
        )
        os.chmod(qualification_path, 0o600)
        self.closed = True

    def write_run_metadata(self, status: str) -> None:
        metadata = {
            "schema": "mc2-frame-trace-run/v1",
            "run_uuid": self.run_uuid,
            "started_unix_ns": self.started_ns,
            "finished_unix_ns": time.time_ns(),
            "status": status,
            "failure": self.failure,
            "observed_state": {
                "epoch": self.candidate_index,
                "candidate_final_tmap_count": self.candidate_tmap_count,
                "skipped_presents": len(self.skipped_presents),
                "armed": self.armed,
                "closed": self.closed,
                "transcript": self.transcript,
            },
        }
        run_path = self.output / "run.json"
        run_path.write_text(
            json.dumps(metadata, sort_keys=True, indent=2) + "\n", encoding="utf-8"
        )
        os.chmod(run_path, 0o600)


OBSERVER = TraceObserver()


class ArmBreakpoint(gdb.Breakpoint):
    def stop(self) -> bool:
        try:
            OBSERVER.begin_candidate()
            return False
        except Exception as error:
            return OBSERVER.fail(f"candidate start failed: {error}")


class TmapBreakpoint(gdb.Breakpoint):
    def stop(self) -> bool:
        if OBSERVER.closed:
            return False
        try:
            selector = OBSERVER.resolve_tmap_selector()
            OBSERVER.record_tmap(selector)
            return False
        except Exception as error:
            return OBSERVER.fail(f"TMAP resolution failed: {error}")


class TerrainBreakpoint(gdb.Breakpoint):
    def __init__(self, location: str, hook: str, **kwargs: object) -> None:
        self.hook = hook
        super().__init__(location, **kwargs)

    def stop(self) -> bool:
        if OBSERVER.closed:
            return False
        try:
            OBSERVER.record_terrain(self.hook)
            return False
        except Exception as error:
            return OBSERVER.fail(f"terrain resolution failed: {error}")


class SkyBreakpoint(gdb.Breakpoint):
    def stop(self) -> bool:
        if OBSERVER.closed:
            return False
        try:
            OBSERVER.record_sky()
            return False
        except Exception as error:
            return OBSERVER.fail(f"sky resolution failed: {error}")


class BitmapBreakpoint(gdb.Breakpoint):
    def __init__(
        self,
        location: str,
        pointer_expression: str,
        hook: str,
        **kwargs: object,
    ) -> None:
        self.pointer_expression = pointer_expression
        self.hook = hook
        super().__init__(location, **kwargs)

    def stop(self) -> bool:
        if OBSERVER.closed:
            return False
        try:
            OBSERVER.record_bitmap(self.pointer_expression, self.hook)
            return False
        except Exception as error:
            return OBSERVER.fail(f"bitmap resolution failed: {error}")


class CursorBreakpoint(gdb.Breakpoint):
    def stop(self) -> bool:
        if OBSERVER.closed:
            return False
        try:
            OBSERVER.record_cursor()
            return False
        except Exception as error:
            return OBSERVER.fail(f"cursor resolution failed: {error}")


class CursorSourceBreakpoint(gdb.Breakpoint):
    def stop(self) -> bool:
        if OBSERVER.closed:
            return False
        try:
            OBSERVER.record_cursor_source()
            return False
        except Exception as error:
            return OBSERVER.fail(f"cursor source resolution failed: {error}")


class TextBreakpoint(gdb.Breakpoint):
    def __init__(
        self,
        location: str,
        pointer_expression: str,
        role: str,
        hook: str,
        **kwargs: object,
    ) -> None:
        self.pointer_expression = pointer_expression
        self.role = role
        self.hook = hook
        super().__init__(location, **kwargs)

    def stop(self) -> bool:
        if OBSERVER.closed:
            return False
        try:
            OBSERVER.record_text(self.pointer_expression, self.role, self.hook)
            return False
        except Exception as error:
            return OBSERVER.fail(f"text resolution failed: {error}")


class GlyphBreakpoint(gdb.Breakpoint):
    def stop(self) -> bool:
        if OBSERVER.closed:
            return False
        try:
            OBSERVER.record_glyph()
            return False
        except Exception as error:
            return OBSERVER.fail(f"glyph resolution failed: {error}")


class SoundBreakpoint(gdb.Breakpoint):
    def stop(self) -> bool:
        if OBSERVER.closed:
            return False
        try:
            OBSERVER.record_sound()
            return False
        except Exception as error:
            return OBSERVER.fail(f"sound resolution failed: {error}")


class MusicBreakpoint(gdb.Breakpoint):
    def stop(self) -> bool:
        if OBSERVER.closed:
            return False
        try:
            OBSERVER.record_music()
            return False
        except Exception as error:
            return OBSERVER.fail(f"music resolution failed: {error}")


class StartupBreakpoint(gdb.Breakpoint):
    def stop(self) -> bool:
        try:
            OBSERVER.capture_startup_hook_resolution()
            return False
        except Exception as error:
            return OBSERVER.fail(f"startup hook resolution failed: {error}")


class SubBlitBreakpoint(gdb.Breakpoint):
    def stop(self) -> bool:
        if OBSERVER.closed:
            return False
        try:
            ordinal = OBSERVER.observe_subblit()
            if ordinal is not None:
                DirectSubBlitFinishBreakpoint(
                    gdb.newest_frame(),
                    ordinal,
                    OBSERVER.candidate_index,
                    internal=True,
                )
            return False
        except Exception as error:
            return OBSERVER.fail(f"SubBlit proof failed: {error}")


class PresentFinishBreakpoint(gdb.FinishBreakpoint):
    def stop(self) -> bool:
        try:
            return OBSERVER.finish_present()
        except Exception as error:
            return OBSERVER.fail(f"frame close failed: {error}")


class DirectSubBlitFinishBreakpoint(gdb.FinishBreakpoint):
    def __init__(
        self, frame: gdb.Frame, ordinal: int, epoch: int, **kwargs: object
    ) -> None:
        self.ordinal = ordinal
        self.epoch = epoch
        super().__init__(frame, **kwargs)

    def stop(self) -> bool:
        try:
            OBSERVER.finish_direct_subblit(self.ordinal, self.epoch)
            return False
        except Exception as error:
            return OBSERVER.fail(f"direct SubBlit return proof failed: {error}")


class VgaBreakpoint(gdb.Breakpoint):
    def stop(self) -> bool:
        if OBSERVER.closed:
            return False
        try:
            if OBSERVER.begin_present(integer("(uintptr_t)$rdi")):
                PresentFinishBreakpoint(gdb.newest_frame(), internal=True)
            return False
        except Exception as error:
            return OBSERVER.fail(f"VGA_Blit classification failed: {error}")


gdb.execute("set pagination off")
gdb.execute("set confirm off")
gdb.execute("set debuginfod enabled off")
gdb.execute("set breakpoint pending on")
ArmBreakpoint("DrawAndEventsInGame_47560", internal=True)
TmapBreakpoint("GameRenderHD::DrawSprite_41BD3", internal=True)
TerrainBreakpoint(
    f"{GAME_RENDER_HD_SOURCE}:2964",
    "GameRenderHD::DrawSquareInProjectionSpace:post_cull",
    internal=True,
)
TerrainBreakpoint(
    f"{GAME_RENDER_HD_SOURCE}:3086",
    "GameRenderHD::DrawInverseSquareInProjectionSpace:post_cull",
    internal=True,
)
SkyBreakpoint("GameRenderHD::DrawSky_40950", internal=True)
BitmapBreakpoint(
    f"{BASIC_SOURCE}:1484",
    "tempposstr.data",
    "DrawBitmap_2BB40",
    internal=True,
)
BitmapBreakpoint(
    f"{GAME_BITMAP_SOURCE}:240",
    "a3.data",
    "GameBitmap::DrawTransparentBitmap_2DE80:low_resolution",
    internal=True,
)
BitmapBreakpoint(
    f"{GAME_BITMAP_SOURCE}:286",
    "a3.data",
    "GameBitmap::DrawTransparentBitmap_2DE80:high_resolution",
    internal=True,
)
BitmapBreakpoint(
    f"{GAME_BITMAP_SOURCE}:408",
    "a3.data",
    "GameBitmap::DrawColourizedBitmap:struct",
    internal=True,
)
CursorSourceBreakpoint(f"{PLAYER_INPUT_SOURCE}:1676", internal=True)
CursorBreakpoint(f"{BASIC_SOURCE}:2063", internal=True)
TextBreakpoint(
    f"{GAME_UI_SOURCE}:3561",
    "text",
    "objective_or_textbox",
    "DrawTextboxText_895D0",
    internal=True,
)
TextBreakpoint(
    f"{BASIC_SOURCE}:1538",
    "textbuffer",
    "raw_draw",
    "DrawText_2BC10",
    internal=True,
)
GlyphBreakpoint(f"{BASIC_SOURCE}:1970", internal=True)
SOUND_BREAKPOINT = SoundBreakpoint(f"{SDL_SOUND_SOURCE}:594", internal=True)
MUSIC_BREAKPOINT = MusicBreakpoint(f"{SDL_SOUND_SOURCE}:104", internal=True)
StartupBreakpoint("main", internal=True, temporary=True)
SubBlitBreakpoint("SubBlit", internal=True)
VgaBreakpoint("VGA_Blit", internal=True)

try:
    gdb.execute("run")
    if OBSERVER.failure:
        raise RuntimeError(OBSERVER.failure)
    if not OBSERVER.closed:
        raise RuntimeError("inferior stopped before the first armed VGA_Blit returned")
    OBSERVER.write_run_metadata("PASS")
except Exception as error:
    if OBSERVER.failure is None:
        OBSERVER.failure = str(error)
    OBSERVER.write_run_metadata("FAIL")
    gdb.write(f"mc2-frame-trace: {OBSERVER.failure}\n", gdb.STDERR)
    gdb.execute("quit 1")
