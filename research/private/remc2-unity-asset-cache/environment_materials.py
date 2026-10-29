"""Package-selected original material banks and their immutable cache products.

Import codecs are passed explicitly from the existing owner importer. No game
callbacks or gameplay admission are implemented in this module.
"""

from __future__ import annotations

import hashlib
from io import BytesIO
import json
from pathlib import Path
import shutil
import struct
import tempfile
from typing import Any, Callable, NamedTuple
import wave


SCHEMA = "mc2-environment-material-cache/v1"
DESCRIPTOR_SCHEMA = "mc2-environment-materials/v1"


class EnvironmentMaterialError(ValueError):
    pass


class ImportCodecs(NamedTuple):
    closure: Callable[..., dict[str, Any]]
    write: Callable[..., dict[str, Any]]
    compile_rnc: Callable[..., Any]
    decode_blob: Callable[..., bytes]
    bitmap_records: Callable[..., Any]
    decode_bitmap: Callable[..., Any]
    proven_bitmap: Callable[..., Any]
    fsync: Callable[..., None]
    output_path: Callable[..., Path]


def canonical(value: Any) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n").encode("utf-8")


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def content_identity(outputs: list[dict[str, Any]]) -> str:
    return digest("".join(f"{row['path']}\0{row['sha256']}\0{row['bytes']}\n" for row in sorted(outputs, key=lambda row: row["path"])).encode("utf-8"))


ENVIRONMENT_AUDIO = {
    0: ("Day", "mc2-day-audio/v1", 0, 2, "C2GAME2", 16_695_296,
        "8c98b86e74a6187a3a08ac34ef81aa3bd97cf493c34bee88b6d6969e79a6c856",
        "9030814e6160f3cd0b9cf5f3c5977715c10c012e9faaacfca3f745c69fa9f5e3"),
    1: ("Night", "mc2-night-audio/v1", 1, 1, "C2GAME1", 19_472_384,
        "0f8081d8ef6ad609f406638862a573b9d1d014f940f0f3194e4d5687f724bd8d",
        "03b3f951048504d8a6d30611d6c44a62ec1c463726b30c33c16e3cb69824605d"),
    2: ("Cave", "mc2-cave-audio/v1", 2, 3, "C2GAME3", 10_584_064,
        "566769008b9598bd03ad657575daa089f217385d7e7671dd948e6b2fea1e3251",
        "4861f36b367de541f3438522cc66520fa8d9a5455aef35639b1bb218388e27a7"),
}
ENVIRONMENT_DAY_SOUND_IDS = (
    1, 2, 3, 5, 8, 9, 10, 12, 13, 14, 15, 16, 18, 19, 23, 29, 30, 31,
    40, 41, 43, 44, 47, 54, 55, 56, 57, 60, 61,
)
ENVIRONMENT_NIGHT_SOUND_IDS = (
    1, 2, 3, 4, 5, 8, 9, 10, 12, 14, 15, 16, 18, 19, 27, 29, 30, 31,
    33, 34, 37, 39, 40, 41, 43, 44, 46, 54, 55, 56, 57, 63,
)


def _environment_music_census(track: int) -> dict[str, Any]:
    division, loops = {
        1: (201, [(0, 1, 116, 0), (158265, 1, 117, 127)]),
        2: (167, [(0, 2, 116, 0)]),
        3: (270, [(3, 9, 116, 0), (86400, 9, 117, 127)]),
        4: (254, [(0, 2, 116, 0), (69117, 2, 117, 127)]),
    }[track]
    tagged = [7, 8] if track in (2, 3) else []
    return {
        "midi_format": 0,
        "track_count": 1,
        "division": division,
        "trigger_controller": 119,
        "tagged_channels": tagged,
        "trigger_events": [
            {"tick": 0, "channel": channel, "controller": 119, "value": 0}
            for channel in tagged
        ],
        "loop_controller_events": [
            {"tick": tick, "channel": channel, "controller": controller, "value": value}
            for tick, channel, controller, value in loops
        ],
    }


def _validate_environment_audio(root: Path, profile: dict[str, Any], by_path: dict[str, dict[str, Any]],
                                source_rows: dict[str, dict[str, Any]]) -> None:
    audio_paths = {path for path in by_path if path.startswith("audio/")}
    if not audio_paths:
        return
    map_id = profile.get("map_type", {}).get("id")
    if map_id not in ENVIRONMENT_AUDIO:
        raise EnvironmentMaterialError("audio outputs have no supported selected environment")
    name, schema, sound_bank, track, label, music_frames, xmi_sha, music_sha = ENVIRONMENT_AUDIO[map_id]
    if (profile.get("map_type", {}).get("name") != name
            or not {"audio/SOUND.DAT", "audio/MUSIC.DAT", "audio/descriptor.json"}.issubset(audio_paths)):
        raise EnvironmentMaterialError("environment audio profile/container set differs")

    output_by_path = by_path
    for path, asset_id in (("audio/SOUND.DAT", "owner_data:SOUND/SOUND.DAT"),
                           ("audio/MUSIC.DAT", "owner_data:SOUND/MUSIC.DAT")):
        output = output_by_path[path]
        source = source_rows.get(asset_id, {})
        payload = (root / path).read_bytes()
        if (output.get("source_asset_ids") != [asset_id]
                or source.get("path") != asset_id.split(":", 1)[1]
                or source.get("size") != len(payload)
                or source.get("content_address") != "sha256:" + digest(payload)
                or output.get("sha256") != digest(payload)):
            raise EnvironmentMaterialError(f"environment audio source/container identity differs: {path}")

    descriptor_path = root / "audio/descriptor.json"
    try:
        descriptor = json.loads(descriptor_path.read_bytes())
    except json.JSONDecodeError as error:
        raise EnvironmentMaterialError("environment audio descriptor is invalid JSON") from error
    full_bank_schema = schema.replace("/v1", "/v2") if map_id in (0, 1) else None
    if (not isinstance(descriptor, dict)
            or (descriptor.get("schema") != schema and
                (full_bank_schema is None or descriptor.get("schema") != full_bank_schema))
            or descriptor.get("profile") != profile):
        raise EnvironmentMaterialError("environment audio descriptor/profile identity differs")
    if output_by_path["audio/descriptor.json"].get("source_asset_ids") != [
        "owner_data:SOUND/SOUND.DAT", "owner_data:SOUND/MUSIC.DAT"
    ]:
        raise EnvironmentMaterialError("environment audio descriptor source bindings differ")
    sound = descriptor.get("sound")
    music = descriptor.get("music")
    expected_ids = (ENVIRONMENT_DAY_SOUND_IDS if map_id == 0 else
                    ENVIRONMENT_NIGHT_SOUND_IDS if map_id == 1 else tuple(range(1, 70)))
    if not isinstance(sound, dict) or sound.get("bank") != sound_bank:
        raise EnvironmentMaterialError("environment audio bank/selector contract differs")
    if (not isinstance(music, dict) or music.get("bank") != 1 or music.get("channel") != 0
            or music.get("driver") != "f" or music.get("map_type") != name
            or music.get("maptype_music") != track):
        raise EnvironmentMaterialError("environment audio bank/selector contract differs")
    # Older Cave v1 audio caches predate the extended source-record metadata.
    # Recognize only that exact historical shape; a current cache cannot drop
    # its selector/hash fields and silently take the compatibility path.
    legacy_cave_audio = (
        map_id == 2
        and set(sound) == {"bank", "decoder", "records"}
        and set(music) == {
            "bank", "channel", "driver", "dynamic_control", "exact_DOS_hardware_timbre",
            "initial_start_status", "logical_track", "loop_scan_pointer_slot", "map_type",
            "maptype_music", "midi_path", "midi_sha256", "optional_random_OGG_path",
            "playback_slot", "records", "unity_wave", "unity_wave_path", "unity_wave_renderer",
            "unity_wave_sha256", "xmi_converter",
        }
        and isinstance(sound.get("records"), list)
        and all(set(row) == {"asset_path", "asset_sha256", "filename", "sound_id", "wav"}
                for row in sound["records"] if isinstance(row, dict))
        and all(isinstance(row, dict) for row in sound["records"])
        and isinstance(music.get("records"), list)
        and all(set(row) == {
            "asset_path", "asset_sha256", "channel", "driver", "filename", "metadata_word",
            "record_bytes", "record_sha256", "relative_data_offset", "slot",
        } for row in music["records"] if isinstance(row, dict))
        and all(isinstance(row, dict) for row in music["records"])
    )
    if (not legacy_cave_audio
            and (sound.get("quality") != 822 or sound.get("quality_index") != 3
                 or sound.get("container_path") != "audio/SOUND.DAT"
                 or sound.get("container_sha256") != output_by_path["audio/SOUND.DAT"].get("sha256"))):
        raise EnvironmentMaterialError("environment audio bank/selector contract differs")
    if (not legacy_cave_audio
            and (music.get("container_path") != "audio/MUSIC.DAT"
                 or music.get("container_sha256") != output_by_path["audio/MUSIC.DAT"].get("sha256"))):
        raise EnvironmentMaterialError("environment audio bank/selector contract differs")
    if (map_id in (0, 1) and
            sound.get("bank_counts_by_quality_index") != [0, 0, 0, 10, 10, 0]):
        raise EnvironmentMaterialError("environment Day/Night sound quality rows differ")
    if map_id == 0 and sound.get("reachable_occurrence_count") != 34:
        raise EnvironmentMaterialError("environment Day reachable audio callsite count differs")

    records = sound.get("records")
    expected_ids = (range(1, 65) if descriptor["schema"] == full_bank_schema
                    else expected_ids)
    if (not isinstance(records, list) or any(not isinstance(row, dict) for row in records)
            or [row.get("sound_id") for row in records] != list(expected_ids)):
        raise EnvironmentMaterialError("environment selected sound record membership/order differs")
    expected_audio_paths = {
        "audio/SOUND.DAT", "audio/MUSIC.DAT", "audio/descriptor.json",
        f"audio/music/{label}.xmi", f"audio/music/{label}.mid",
        f"audio/music/{label}-adlmidi-bank59.wav",
    }
    for row in records:
        sound_id = row["sound_id"]
        record_bytes = row.get("record_bytes")
        record_sha256 = row.get("record_sha256")
        if (not isinstance(row.get("filename"), str) or not row["filename"]
                or (not legacy_cave_audio and row.get("bank") != sound_bank)
                or (not legacy_cave_audio and row.get("metadata_slot") != sound_id - 1)
                or (not legacy_cave_audio and
                    (not isinstance(record_bytes, int) or record_bytes <= 0
                     or not isinstance(record_sha256, str) or len(record_sha256) != 64))
                or (map_id == 1 and row.get("metadata_word") != 90)):
            raise EnvironmentMaterialError(f"environment sound record provenance differs: {sound_id}")
        if map_id == 1 and sound_id == 12:
            if (row.get("mapping_status") != "explicit_null" or row.get("asset_path") is not None
                    or row.get("asset_sha256") is not None or row.get("wav") is not None
                    or row.get("play_class") != "explicit_null"
                    or row.get("playback_scope") != "explicit_null"
                    or row.get("loop_target_volume") != 0
                    or row.get("semantic_name") != Path(row["filename"]).stem.lower()):
                raise EnvironmentMaterialError("environment Night explicit-null sound differs")
            continue
        asset_path = f"audio/sfx/{sound_id:03d}.wav"
        output = output_by_path.get(asset_path, {})
        wave_row = row.get("wav", {})
        if (not isinstance(wave_row, dict)
                or row.get("asset_path") != asset_path or row.get("asset_sha256") != output.get("sha256")
                or (not legacy_cave_audio and
                    (record_bytes != output.get("bytes")
                     or record_sha256 != output.get("sha256")))
                or output.get("source_asset_ids") != ["owner_data:SOUND/SOUND.DAT"]
                or wave_row.get("channels") != 1 or wave_row.get("sample_width_bytes") != 1
                or wave_row.get("sample_rate") != 22050 or wave_row.get("frame_count", 0) <= 0):
            raise EnvironmentMaterialError(f"environment sound source/output binding differs: {sound_id}")
        try:
            with wave.open(BytesIO((root / asset_path).read_bytes()), "rb") as wav:
                actual_wave = (wav.getnchannels(), wav.getsampwidth(), wav.getframerate(), wav.getnframes())
        except (wave.Error, EOFError) as error:
            raise EnvironmentMaterialError(f"environment sound is not a readable WAVE: {sound_id}") from error
        if actual_wave != (1, 1, 22050, wave_row["frame_count"]):
            raise EnvironmentMaterialError(f"environment sound WAVE format differs: {sound_id}")
        if map_id == 1:
            expected_scope = ("local_ambient" if sound_id in (1, 2, 5, 31)
                              else "local_feedback" if sound_id in (14, 29)
                              else "original_stereo_positional")
            expected_class = "loop" if sound_id in (1, 2, 5, 31) else "one_shot"
            expected_volume = {1: 70, 2: 70, 5: 120, 31: 85}.get(sound_id, 127)
            if (row.get("mapping_status") != "proven_campaign_Night"
                    or row.get("playback_scope") != expected_scope
                    or row.get("play_class") != expected_class
                    or row.get("loop_target_volume") != expected_volume
                    or row.get("semantic_name") != Path(row["filename"]).stem.lower()):
                raise EnvironmentMaterialError(f"environment Night sound semantics differ: {sound_id}")
        expected_audio_paths.add(asset_path)

    music_rows = music.get("records")
    if (not isinstance(music_rows, list) or len(music_rows) != 6
            or any(not isinstance(row, dict) for row in music_rows)):
        raise EnvironmentMaterialError("environment MUSIC record census is incomplete")
    if (not legacy_cave_audio and
            sound.get("record_census_sha256") != digest(canonical(records))):
        raise EnvironmentMaterialError("environment sound record census hash differs")
    if (not legacy_cave_audio and
            music.get("record_census_sha256") != digest(canonical(music_rows))):
        raise EnvironmentMaterialError("environment music record census hash differs")
    expected_names = ["C2GAME1.XMI", "C2GAME2.XMI", "C2GAME3.XMI", "C2SETUP.XMI", "C2INTRO.XMI", "C2CUTS.XMI"]
    for slot, row in enumerate(music_rows):
        if (row.get("slot") != slot or row.get("filename") != expected_names[slot]
                or row.get("metadata_word") != 90
                or (not legacy_cave_audio and
                    (not isinstance(row.get("record_bytes"), int)
                     or row["record_bytes"] <= 0
                     or not isinstance(row.get("record_sha256"), str)
                     or len(row["record_sha256"]) != 64))):
            raise EnvironmentMaterialError("environment MUSIC source record order differs")
        if slot == track - 1:
            asset_path = f"audio/music/{label}.xmi"
            output = output_by_path.get(asset_path, {})
            if (row.get("asset_path") != asset_path or row.get("asset_sha256") != xmi_sha
                    or (not legacy_cave_audio and
                        (row.get("record_bytes") != output.get("bytes")
                         or row.get("record_sha256") != output.get("sha256")))
                    or output.get("sha256") != xmi_sha
                    or output.get("source_asset_ids") != ["owner_data:SOUND/MUSIC.DAT"]):
                raise EnvironmentMaterialError("environment selected MUSIC source/output record differs")
        elif row.get("asset_path") is not None or row.get("asset_sha256") is not None:
            raise EnvironmentMaterialError("environment contains an unselected MUSIC payload")

    music_path = f"audio/music/{label}-adlmidi-bank59.wav"
    music_output = output_by_path.get(music_path, {})
    music_wave = music.get("unity_wave", {})
    if (not isinstance(music_wave, dict)
            or music.get("logical_track") != track or music.get("playback_slot") != track - 1
            or music.get("loop_scan_pointer_slot") != track
            or music.get("unity_wave_path") != music_path
            or music.get("unity_wave_sha256") != music_sha
            or music_output.get("sha256") != music_sha
            or music_output.get("source_asset_ids") != ["owner_data:SOUND/MUSIC.DAT"]
            or music_wave.get("channels") != 2 or music_wave.get("sample_width_bytes") != 2
            or music_wave.get("sample_rate") != 44100 or music_wave.get("frame_count") != music_frames):
        raise EnvironmentMaterialError("environment selected music output binding differs")
    if (music.get("initial_start_status") != "SourceOrderQualified_remc2_local_gameTurn1_after_second_DrawAndEvents"
            or music.get("exact_DOS_hardware_timbre") != "Deferred"
            or music.get("optional_random_OGG_path") != "Excluded"):
        raise EnvironmentMaterialError("environment source music lifecycle contract differs")
    control = music.get("dynamic_control", {})
    if not isinstance(control, dict):
        raise EnvironmentMaterialError("environment source music controller descriptor is missing")
    census = lambda selected: _environment_music_census(selected)
    if (control.get("playback_slot_census") != census(track)
            or control.get("loop_scan_pointer_census") != census(track + 1)
            or control.get("audible_channel_resolution") != (
                "Unresolved_playback_slot0_has_no_CC119_but_loop_scan_slot1_tags_channels7_8"
                if track == 1 else "Source_playback_track_minus_one_loop_scan_track")
            or control.get("timer_hz") != 90 or control.get("timer_multiplier") != 3
            or control.get("timer_hz_formula") != "30 * timer_multiplier"
            or control.get("envelope_min") != 0 or control.get("envelope_max") != 127
            or control.get("envelope_step_per_callback") != 1
            or control.get("update_mode_field") != "target_mode"):
        raise EnvironmentMaterialError("environment source music controller census differs")
    if audio_paths != expected_audio_paths:
        raise EnvironmentMaterialError("environment audio output set has extra or missing files")


def build(owner_root: Path, package: Path, catalog_path: Path, output: Path, codecs: ImportCodecs) -> dict[str, Any]:
    owner_root = owner_root.expanduser().resolve(strict=True)
    package = package.expanduser().resolve(strict=True)
    output = codecs.output_path(output)
    # A live generation's cache is immutable. Never replace an existing tree.
    if output.exists():
        raise EnvironmentMaterialError("environment cache output already exists")
    closure = codecs.closure(package, catalog_path, owner_root)
    profile = closure["profile"]
    catalog = json.loads(catalog_path.read_bytes())
    entries = {row["asset_id"]: row for row in catalog["entries"]}
    sources = {}
    for row in closure["owner_files"]:
        path = owner_root / row["path"]
        if path.is_symlink() or not path.resolve(strict=True).is_relative_to(owner_root):
            raise EnvironmentMaterialError("environment source escapes owner root")
        data = path.read_bytes()
        if len(data) != row["size"] or "sha256:" + digest(data) != row["content_address"]:
            raise EnvironmentMaterialError(f"owner source changed: {row['path']}")
        sources[row["path"]] = data
    output.parent.mkdir(parents=True, exist_ok=True)
    stage = Path(tempfile.mkdtemp(prefix="environment-material-", dir=output.parent))
    outputs: list[dict[str, Any]] = []

    def emit(path: str, data: bytes, kind: str, asset: str, **metadata: Any) -> dict[str, Any]:
        return codecs.write(stage, outputs, path, data, kind=kind, source_asset_ids=[asset], extra=metadata or None)

    def raw(bank: str) -> bytes:
        return sources[f"DATA/{bank}.DAT"]

    palette_id = f"owner_data:DATA/{profile['palette']}.DAT"
    palette = raw(profile["palette"])
    if len(palette) != 768 or max(palette) > 63:
        raise EnvironmentMaterialError("selected palette is not 256 RGB6 triplets")
    palette_row = emit("palette/source.rgb6", palette, "ordered_rgb6_palette", palette_id)
    rgb = emit("palette/source.rgb8", bytes(value * 4 for value in palette), "ordered_rgb8_palette_exact_vga_scale", palette_id)
    tables = raw(profile["tables"])
    colors = raw(profile["colors"])
    if len(tables) != 83_456 or len(colors) != 4_096:
        raise EnvironmentMaterialError("selected shade/blend or colour table extent differs")
    tables_row = emit("palette/tables.u8", tables, "original_renderer_tables_u8", f"owner_data:DATA/{profile['tables']}.DAT")
    colors_row = emit("palette/colors.u8", colors, "original_rgb444_lookup_u8", f"owner_data:DATA/{profile['colors']}.DAT")
    atlas = raw(profile["blocks"])
    if len(atlas) != 152 * 32 * 32:
        raise EnvironmentMaterialError("selected source32 block atlas extent differs")
    atlas_row = emit("terrain/blocks.atlas.u8", atlas, "indexed_atlas_u8", f"owner_data:DATA/{profile['blocks']}.DAT", width=256, height=608)
    variant = emit("terrain/source16.dat", raw(profile["dormant_blocks16"]), "dormant_original_texture16_bank", f"owner_data:DATA/{profile['dormant_blocks16']}.DAT")
    sky_row = None
    if profile["sky"] is not None:
        sky = raw(profile["sky"])
        if len(sky) != 65_536:
            raise EnvironmentMaterialError("selected source32 sky extent differs")
        sky_row = emit("sky/source.u8", sky, "indexed_image_u8", f"owner_data:DATA/{profile['sky']}.DAT", width=256, height=256)
    exit_tables = None
    if profile["map_type"]["id"] == 0:
        exit_tables = emit("palette/day-exit-gtd2.dat", raw("GTD2"), "original_day_exit_tables", "owner_data:DATA/GTD2.DAT")
    scratch = stage / "decoder"
    scratch.mkdir()
    decoder, decoder_identity = codecs.compile_rnc(scratch)
    bank = profile["tmaps"]
    tmap_data = raw(bank)
    tab = sources[f"DATA/{bank}.TAB"]
    if len(tab) != 505 * 10:
        raise EnvironmentMaterialError("selected TMAP table requires 504 rows and adjacent sentinel")
    rows = list(struct.iter_unpack("<iiH", tab))
    textures = []
    for index, (decoded_size, offset, group) in enumerate(rows[:-1]):
        end = rows[index + 1][1]
        asset_id = f"owner_data:DATA/{bank}.DAT#{index}"
        entry = entries.get(asset_id, {})
        if not 0 <= offset < end <= len(tmap_data) or entry.get("offset") != offset or entry.get("end_offset") != end or entry.get("metadata_dword_0") != decoded_size or entry.get("animation_or_allocation_group") != group or entry.get("packed_span_sha256") != digest(tmap_data[offset:end]):
            raise EnvironmentMaterialError(f"TMAP adjacent span/catalog mismatch: {asset_id}")
        packed = tmap_data[offset:end]
        decoded = codecs.decode_blob(decoder, packed, 0, decoded_size, scratch, f"tmap-{index}")
        if decoded == b"0":
            payload = emit(f"textures/{index:03d}.decoded", decoded, "original_empty_tmap_payload", asset_id)
            textures.append({"index": index, "group": group, "source_empty": True,
                "decoded_payload_path": payload["path"], "source_asset_id": asset_id})
            continue
        if len(decoded) < 6:
            raise EnvironmentMaterialError(f"TMAP header truncated: {asset_id}")
        header, width, height = struct.unpack_from("<HHH", decoded)
        frame_end = 6 + width * height
        if not width or not height or frame_end > len(decoded):
            raise EnvironmentMaterialError(f"TMAP dimensions exceed decoded payload: {asset_id}")
        frame_count = 1
        if header & 1:
            if frame_end + 2 > len(decoded):
                raise EnvironmentMaterialError(f"animated TMAP has no frame stream: {asset_id}")
            frame_count = struct.unpack_from("<H", decoded, frame_end)[0]
            if not frame_count:
                raise EnvironmentMaterialError(f"animated TMAP has zero frames: {asset_id}")
        elif frame_end != len(decoded):
            raise EnvironmentMaterialError(f"static TMAP has trailing bytes: {asset_id}")
        payload = emit(f"textures/{index:03d}.decoded", decoded, "original_tmap_decoded_payload", asset_id)
        pixels = emit(f"textures/{index:03d}.indices.u8", decoded[6:frame_end], "indexed_image_u8", asset_id)
        textures.append({"index": index, "group": group, "width": width, "height": height, "animated": bool(header & 1), "frame_count": frame_count, "decoded_payload_path": payload["path"], "indices_path": pixels["path"], "source_asset_id": asset_id})
    shutil.rmtree(scratch)  # Known disposable codec scratch, never imported data.
    bitmap_banks = []
    proven_bitmap = codecs.proven_bitmap()
    for bank in (profile["sprites"], profile["web"], "FONT1"):
        data = raw(bank)
        records = codecs.bitmap_records(sources[f"DATA/{bank}.TAB"])
        bitmaps = []
        for selector, (offset, width, height) in enumerate(records):
            row = {"selector": selector, "width": width, "height": height}
            if not width or not height:
                row["source_empty"] = True
            else:
                asset = f"owner_data:DATA/{bank}.DAT#{selector}"
                pixels, mask, width, height, _ = codecs.decode_bitmap(data, records, selector, entries.get(asset, {}), proven_bitmap, bank)
                row["indices_path"] = emit(f"bitmaps/{bank.lower()}/{selector:03d}.indices.u8", pixels, "indexed_bitmap_u8", asset)["path"]
                row["mask_path"] = emit(f"bitmaps/{bank.lower()}/{selector:03d}.mask.u8", mask, "source_opaque_coverage_u8", asset)["path"]
            bitmaps.append(row)
        bitmap_banks.append({"bank": bank, "bitmaps": bitmaps})
    descriptor = {
        "schema": DESCRIPTOR_SCHEMA, "profile": profile,
        "palette": {"path": palette_row["path"], "rgb8_path": rgb["path"]},
        "tables": {"path": tables_row["path"], "key_color_1": profile["key_color_1"], "key_color_2": profile["key_color_2"]},
        "colors": {"path": colors_row["path"]},
        "blocks": {"atlas_path": atlas_row["path"], "width": 256, "height": 608, "tile_size": 32, "tile_count": 152},
        "source16": {"path": variant["path"], "activation": "dormant-source-variant"},
        "sky": None if sky_row is None else {"path": sky_row["path"], "width": 256, "height": 256},
        "day_exit_tables": None if exit_tables is None else {"path": exit_tables["path"]},
        "tmaps": {"bank": profile["tmaps"], "textures": textures}, "bitmap_banks": bitmap_banks,
    }
    emit("materials/descriptor.json", canonical(descriptor), "canonical_environment_material_descriptor", "owner_data:DATA/" + profile["tables"] + ".DAT")
    emit("materials/closure.json", canonical(closure), "canonical_environment_asset_closure", "owner_data:DATA/" + profile["tmaps"] + ".DAT")
    manifest = {"schema": SCHEMA, "profile": profile, "closure_sha256": closure["closure_sha256"], "source_files": closure["owner_files"], "decoder": decoder_identity, "outputs": outputs, "cache_content_sha256": content_identity(outputs)}
    manifest["manifest_payload_sha256"] = digest(canonical(manifest))
    (stage / "manifest.json").write_bytes(canonical(manifest))
    validate(stage)
    codecs.fsync(stage)
    stage.rename(output)
    return manifest


def validate(root: Path) -> dict[str, Any]:
    manifest = json.loads((root / "manifest.json").read_bytes())
    unsigned = dict(manifest)
    claim = unsigned.pop("manifest_payload_sha256", None)
    if manifest.get("schema") != SCHEMA or claim != digest(canonical(unsigned)):
        raise EnvironmentMaterialError("environment manifest schema/metadata identity differs")
    outputs = manifest.get("outputs", [])
    if content_identity(outputs) != manifest.get("cache_content_sha256"):
        raise EnvironmentMaterialError("environment content identity differs")
    seen = set()
    for row in outputs:
        relative = Path(row["path"])
        path = root / relative
        if relative.is_absolute() or ".." in relative.parts or row["path"] in seen or path.is_symlink() or not path.resolve(strict=True).is_relative_to(root.resolve()):
            raise EnvironmentMaterialError("environment output path is invalid/duplicate")
        seen.add(row["path"])
        payload = path.read_bytes()
        if len(payload) != row["bytes"] or digest(payload) != row["sha256"]:
            raise EnvironmentMaterialError(f"environment output identity differs: {row['path']}")
    actual = {path.relative_to(root).as_posix() for path in root.rglob("*") if path.is_file()}
    if actual != seen | {"manifest.json"}:
        raise EnvironmentMaterialError("environment cache has unlisted or missing outputs")
    descriptor = json.loads((root / "materials/descriptor.json").read_bytes())
    if descriptor.get("schema") != DESCRIPTOR_SCHEMA or descriptor.get("profile") != manifest["profile"]:
        raise EnvironmentMaterialError("environment descriptor/profile identity differs")
    if (descriptor["sky"] is None) != (manifest["profile"]["map_type"]["id"] == 2):
        raise EnvironmentMaterialError("Cave sky policy differs from source")
    profile = manifest["profile"]
    map_id = profile["map_type"]["id"]
    letter = {0: "D", 1: "N", 2: "C"}[map_id]
    display = "F" if map_id == 1 and profile["terrain_graphics_flags"] & 2 else letter
    prefix = "M" if profile["vga_resolution_selector"] == 1 else "H"
    expected = {"palette": f"PAL{display}-0", "tables": f"TABLES{letter}",
        "colors": f"CLR{letter}-0", "blocks": "BLOCK32" if map_id == 0 else f"BL32{display}0-0",
        "dormant_blocks16": "BLOCK16" if map_id == 0 else f"BL16{display}0-0",
        "sky": None if map_id == 2 else f"SKY{letter}0-0", "tmaps": f"TMAPS{map_id}-0",
        "sprites": f"{prefix}SPR{letter}0-0", "web": f"{prefix}WEB{letter}0-0",
        "key_color_1": 0 if map_id == 1 else 254, "key_color_2": 0 if map_id == 0 else 255,
        "texture_size": 32}
    if profile["vga_resolution_selector"] not in (0, 1) or any(profile.get(key) != value for key, value in expected.items()):
        raise EnvironmentMaterialError("environment selectors differ from source")
    by_path = {row["path"]: row for row in outputs}
    def payload(path: str) -> bytes:
        if path not in by_path:
            raise EnvironmentMaterialError("environment descriptor references an unlisted output")
        return (root / path).read_bytes()
    source_ids = {row["asset_id"] for row in manifest["source_files"]}
    source_rows = {row["asset_id"]: row for row in manifest["source_files"]}
    for row in outputs:
        if not row["source_asset_ids"] or any(asset.split("#")[0] not in source_ids for asset in row["source_asset_ids"]):
            raise EnvironmentMaterialError("environment product references unretained source")
    raw_products = {"palette/source.rgb6": profile["palette"], "palette/tables.u8": profile["tables"],
        "palette/colors.u8": profile["colors"], "terrain/blocks.atlas.u8": profile["blocks"],
        "terrain/source16.dat": profile["dormant_blocks16"]}
    if profile["sky"] is not None:
        raw_products["sky/source.u8"] = profile["sky"]
    if map_id == 0:
        raw_products["palette/day-exit-gtd2.dat"] = "GTD2"
    for path, bank in raw_products.items():
        source_id = f"owner_data:DATA/{bank}.DAT"
        row, source = by_path.get(path, {}), source_rows.get(source_id, {})
        if row.get("source_asset_ids") != [source_id] or source.get("content_address") != "sha256:" + row.get("sha256", "") or source.get("size") != row.get("bytes"):
            raise EnvironmentMaterialError(f"selected owner bank identity differs: {bank}")
    for path, extent in {"palette/source.rgb6": 768, "palette/source.rgb8": 768,
            "palette/tables.u8": 83456, "palette/colors.u8": 4096, "terrain/blocks.atlas.u8": 155648}.items():
        if by_path.get(path, {}).get("bytes") != extent:
            raise EnvironmentMaterialError(f"environment resource extent differs: {path}")
    six = (root / "palette/source.rgb6").read_bytes()
    if max(six) > 63 or bytes(value * 4 for value in six) != (root / "palette/source.rgb8").read_bytes():
        raise EnvironmentMaterialError("environment palette conversion differs")
    textures = descriptor["tmaps"]["textures"]
    if descriptor["tmaps"]["bank"] != profile["tmaps"] or [row["index"] for row in textures] != list(range(504)):
        raise EnvironmentMaterialError("environment TMAP table is incomplete")
    for row in textures:
        data = payload(row["decoded_payload_path"])
        if row.get("source_empty"):
            if data != b"0" or "indices_path" in row:
                raise EnvironmentMaterialError("environment empty TMAP differs")
        else:
            header, width, height = struct.unpack_from("<HHH", data)
            if width != row["width"] or height != row["height"] or not width * height or data[6:6 + width * height] != payload(row["indices_path"]):
                raise EnvironmentMaterialError("environment TMAP dimensions/pixels differ")
    if {bank["bank"] for bank in descriptor["bitmap_banks"]} != {profile["sprites"], profile["web"], "FONT1"}:
        raise EnvironmentMaterialError("environment bitmap banks differ")
    for bank in descriptor["bitmap_banks"]:
        if [row["selector"] for row in bank["bitmaps"]] != list(range(len(bank["bitmaps"]))):
            raise EnvironmentMaterialError("environment bitmap order differs")
        for row in bank["bitmaps"]:
            if row.get("source_empty"):
                if row["width"] and row["height"]:
                    raise EnvironmentMaterialError("nonempty bitmap marked empty")
                continue
            pixels = payload(row["indices_path"])
            mask = payload(row["mask_path"])
            if len(pixels) != row["width"] * row["height"] or len(mask) != len(pixels) or max(mask) > 1:
                raise EnvironmentMaterialError("environment bitmap extent/coverage differs")
    _validate_environment_audio(root, profile, by_path, source_rows)
    return manifest
