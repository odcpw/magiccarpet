#!/usr/bin/env python3
"""Build a private, evidence-linked semantic crosswalk for remc2/MC2.

The source image is never opened by this script. It consumes only the small,
hash-verified owner-data evidence set extracted under ``semantics/owned-assets``
plus pinned remc2 source and its language-maintenance CSV.
"""

from __future__ import annotations

import csv
import hashlib
import json
import re
import struct
from dataclasses import asdict, dataclass
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


HERE = Path(__file__).resolve().parent
LAB_ROOT = HERE.parents[2]
UPSTREAM = LAB_ROOT / "upstream/magic-carpet-2-hd"
ENGINE = UPSTREAM / "remc2/engine"
SEMANTICS = HERE / "semantics"
OWNED = SEMANTICS / "owned-assets"
OUTPUT = SEMANTICS / "generated"
LANGUAGE_CSV = UPSTREAM / "tools/makeplremc2/makeczmc2/d2-pl.csv"
HIGH_RES = UPSTREAM / "enhancedassets/graphics/high-res"

FAMILY_NAMES = (
    "Fireball",
    "Possession",
    "Castle",
    "Speed Up",
    "Metamorph",
    "Heal",
    "Shield",
    "Lightning",
    "Rebound",
    "Meteor",
    "Teleport",
    "Invisible",
    "Beyond Sight",
    "Steal Mana",
    "Duel",
    "Tremor",
    "Crater",
    "Earthquake",
    "Volcano",
    "Summon Army",
    "Gravity Well",
    "Whirlwind",
    "Fool's Mana",
    "Magic Mine",
    "Alliance",
    "Cave In",
)

MAP_MODES = {
    "day": ("HSPRD0-0", "PALD-0.DAT"),
    "night": ("HSPRN0-0", "PALN-0.DAT"),
    "cave": ("HSPRC0-0", "PALC-0.DAT"),
}

SPELL_RECORD = struct.Struct("<iiiii h h b B")

RUNTIME_CLASSES = (
    (0, "free or inactive entity slot", "STRONG", "allocator and lifecycle", "free-list member, not a live entity"),
    (1, "reserved addressless records", "STRONG", "action/constructor table is addressless", "purpose unresolved"),
    (2, "scenery, ambient objects, and fauna", "STRONG", "constructors and action table", ""),
    (3, "players, castles, balloons, and global-position markers", "STRONG", "constructors and action table", ""),
    (4, "empty class", "EXACT_STATIC", "empty class registry entry", ""),
    (5, "living actors, creatures, and civilians", "STRONG", "29-slot constructor and action tables", ""),
    (6, "empty class", "EXACT_STATIC", "empty class registry entry", ""),
    (7, "empty class; legacy Weather labels are not live dispatch", "EXACT_STATIC", "empty class registry entry", ""),
    (8, "empty class", "EXACT_STATIC", "empty class registry entry", ""),
    (9, "projectiles and spell delivery entities", "STRONG", "constructors, actions, and casting consumers", ""),
    (10, "impacts, particles, terrain effects, mana spheres, and buildings", "STRONG", "constructors, actions, and world consumers", ""),
    (11, "switches, checkpoints, and triggers", "STRONG", "constructor writes and action table", "corrects stale spell comment"),
    (12, "empty class", "EXACT_STATIC", "empty class registry entry", ""),
    (13, "reserved addressless metadata", "STRONG", "addressless table rows", "purpose unresolved"),
    (14, "special world, level, and UI fixtures", "STRONG", "constructors and action table", ""),
    (15, "spell-holder and inventory entities", "STRONG", "spell-holder constructors and writes", "corrects stale class-11 spell comment"),
)

# Raw runtime class-5 model order. Friendly identity never comes from the
# positional text list alone; identity_basis records a causally consumed
# constructor/action/sound asset or leaves the label explicitly unresolved.
CREATURE_MODELS = (
    (0, "Dragon", True, 1, 80, 16, 4000, "2250/4500", "sub_4B240", "33650-33726", "STRONG", "multipart constructor and Dragon sound consumers", "head starts action 1; sixteen linked body segments use exceptional action 232"),
    (1, "Goat", True, 9, 54, 18, 600, "", "AddCreature_4B490", "33729-33756", "CONFIRMED", "named goat actions and Goat_46 sound", "recovered constructor comment says Vulture and is contradicted by consumers"),
    (2, "Bee", True, 17, 64, 30, 3000, "", "sub_4B590", "33759-33803", "STRONG", "day-only constructor and Bee sounds", ""),
    (3, "unresolved worm-like multipart creature", True, 25, 64, 16, 9000, "", "sub_4B6F0", "33805-33884", "PROVISIONAL", "multipart constructor behavior", "head starts action 25; sixteen linked body segments use exceptional action 232; friendly identity unresolved"),
    (4, "Archer", True, 33, 30, 0, 1000, "", "AddArchers_4BA10", "33887-33916", "STRONG", "named constructor and action family", ""),
    (5, "disabled or cut model 5", False, "", "", "", "", "", "", "1481-1512", "EXACT_STATIC", "zero constructor-table entry", "no discovered runtime creation path"),
    (6, "disabled or cut model 6", False, "", "", "", "", "", "", "1481-1512", "EXACT_STATIC", "zero constructor-table entry", "no discovered runtime creation path"),
    (7, "disabled or cut model 7", False, "", "", "", "", "", "", "1481-1512", "EXACT_STATIC", "zero constructor-table entry", "no discovered runtime creation path"),
    (8, "disabled or cut model 8", False, "", "", "", "", "", "", "1481-1512", "EXACT_STATIC", "zero constructor-table entry", "no discovered runtime creation path"),
    (9, "Skeleton candidate", True, 72, 20, 0, 1000, "", "sub_4BBB0", "33920-33969", "PROVISIONAL", "constructor/action behavior", "needs causally consumed sprite or sound confirmation"),
    (10, "Vissuluth candidate", True, 80, 0, 0, 300000, "", "sub_4BD00", "33973-34003", "PROVISIONAL", "stationary terrain-gated constructor", "friendly identity not asset-confirmed"),
    (11, "disabled or cut model 11", False, "", "", "", "", "", "", "1481-1512", "EXACT_STATIC", "zero constructor-table entry", "no discovered runtime creation path"),
    (12, "Builder candidate", True, 97, 54, 24, 1000, "", "sub_4BDF0", "34007-34041", "PROVISIONAL", "constructor/action behavior", "friendly identity not asset-confirmed"),
    (13, "Townie or villager", True, 105, 54, 18, 1000, "", "AddVilliger_4BF40", "34045-34099", "STRONG", "named constructor and civilian behavior", ""),
    (14, "Trader", True, 113, 54, 18, 1000, "", "AddTrader_4C0B0", "34102-34134", "STRONG", "named constructor and economy behavior", ""),
    (15, "unresolved stationary actor", True, 121, 30, 0, 1000, "", "sub_4C1E0", "34137-34167", "PROVISIONAL", "constructor behavior", "friendly identity unresolved"),
    (16, "Wyvern", True, 129, 60, 20, 60000, "", "sub_4C310", "34171-34205", "STRONG", "Wyvern sound consumer", ""),
    (17, "Mad-Dog sound-family actor", True, 137, 68, 20, 10000, "", "sub_4C460", "34209-34240", "PROVISIONAL", "Mad-Dog sound consumer", "may be Manticore; final creature identity unresolved"),
    (18, "unresolved model 18", True, 147, 10, 6, 36000, "", "sub_4C590", "34244-34275", "PROVISIONAL", "constructor behavior", "friendly identity unresolved"),
    (19, "Firefly or Firebug", True, 153, 76, 8, 600, "", "sub_4C6B0", "34279-34311", "STRONG", "named action family and asset consumers", ""),
    (20, "Spider", True, 161, 32, 20, 5500, "", "sub_4C7F0", "34315-34345", "STRONG", "Spider sound consumer", ""),
    (21, "Devil", True, 169, 96, "", 1000, "1000", "sub_4C8F0", "34348-34381", "STRONG", "Devil sound consumer", ""),
    (22, "Mana Worm", True, 176, 128, 16, 2000, "", "sub_4CA00", "34385-34424", "STRONG", "MWorm3_48 sound consumer", ""),
    (23, "MdWeller sound-family actor", True, 184, 24, 14, 10000, "100", "sub_4CBF0", "34462-34493", "PROVISIONAL", "MdWeller sound consumer", "final friendly identity unresolved"),
    (24, "Caveman sound-family actor", True, 193, 80, 24, 16000, "", "sub_4CCF0", "34495-34529", "STRONG", "cave-only constructor and Caveman sound consumer", ""),
    (25, "Cymmerian", True, 201, 60, 20, 7500, "", "sub_4CE00", "34531-34563", "STRONG", "Cymmerian sound consumer", ""),
    (26, "Zombie", True, 209, 25, 25, 4400, "", "sub_4CF00", "34565-34597", "CONFIRMED", "Zombie_62 sound consumer", "corrects positional text-list model 29"),
    (27, "Hydra", True, 217, "", "", "", "", "sub_4D000", "34599-34701", "STRONG", "46-object multipart construction and Hydra sound consumers", ""),
    (28, "Deep One", True, 225, 120, 64, 8000, "", "sub_4D1D0", "34703-34735", "STRONG", "DeepOne_38 action/sound consumers", ""),
)

CREATURE_CONSTRUCTOR_ADDRESSES = {
    0: "0x0022C240", 1: "0x0022C490", 2: "0x0022C590", 3: "0x0022C6F0",
    4: "0x0022CA10", 5: "0x0022CB60", 6: "0x0022CB70", 7: "0x0022CB90",
    8: "0x0022CBA0", 9: "0x0022CBB0", 10: "0x0022CD00", 11: "0x0022CDE0",
    12: "0x0022CDF0", 13: "0x0022CF40", 14: "0x0022D0B0", 15: "0x0022D1E0",
    16: "0x0022D310", 17: "0x0022D460", 18: "0x0022D590", 19: "0x0022D6B0",
    20: "0x0022D7F0", 21: "0x0022D8F0", 22: "0x0022DA00", 23: "0x0022DBF0",
    24: "0x0022DCF0", 25: "0x0022DE00", 26: "0x0022DF00", 27: "0x0022E000",
    28: "0x0022E1D0",
}

CREATURE_EXTRA_ANCHORS = {
    0: "EventsFunctions.cpp:11206",
    1: "EventsFunctions.cpp:11386-11461; WavIndexes.h:50",
    2: "WavIndexes.h:16-17",
    4: "EventsFunctions.cpp:11885-11940",
    16: "EventsFunctions.cpp:15389-15503; WavIndexes.h:43",
    17: "EventsFunctions.cpp:15596-15613",
    19: "EventsFunctions.cpp:16273-16613",
    20: "EventsFunctions.cpp:16649-16667",
    21: "EventsFunctions.cpp:17011-17075",
    22: "EventsFunctions.cpp:17720-17737",
    23: "EventsFunctions.cpp:18211-18310",
    24: "EventsFunctions.cpp:18680-18682",
    25: "EventsFunctions.cpp:19076-19080",
    26: "EventsFunctions.cpp:19233-19255",
    27: "EventsFunctions.cpp:19737-20202",
    28: "EventsFunctions.cpp:21010-21127",
}


@dataclass(frozen=True)
class TierRecord:
    family_id: int
    family_name: str
    family_display_name: str
    family_text_id: int
    family_header_tier_count: int
    family_initial_flags_raw: int
    tier: int
    tier_name: str
    tier_text_id: int
    alternate_tier_name: str
    alternate_tier_text_id: str
    effect_selector_raw: int
    mana_cost: int
    max_mana_requirement: int
    single_player_experience: int
    multiplayer_experience: int
    hint_text_id_loaded: int
    timing_count_raw: int
    life_selector_raw: int
    tier_flags_raw: int
    family_icon_small_index: int
    family_icon_big_index: int
    tier_icon_index: int
    load_override_chain: str
    confidence: str
    extraction_ready: bool


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def slug(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", value.casefold()).strip("-")


def read_english_strings() -> dict[int, str]:
    strings: dict[int, str] = {}
    with LANGUAGE_CSV.open("r", encoding="latin-1", newline="") as handle:
        for raw in handle:
            parts = raw.rstrip("\r\n").split("#")
            if len(parts) < 3 or not parts[-2].isdigit():
                continue
            text_id = int(parts[-2])
            english = parts[0].strip()
            if english and not set(english) <= {"X", "\ufffd"}:
                strings[text_id] = english
    return strings


def tier_text_ids(family_id: int, tier: int) -> tuple[int, int | None]:
    if family_id < 4:
        return 186 + family_id * 3 + tier, None
    if family_id == 4:
        return ((198, 199) if tier == 0 else (199 + tier, None))
    if family_id < 19:
        return 202 + (family_id - 5) * 3 + tier, None
    if family_id == 19:
        return ((244, 245) if tier == 0 else (245 + tier, None))
    return 248 + (family_id - 20) * 3 + tier, None


def read_spell_table(strings: dict[int, str]) -> list[TierRecord]:
    path = OWNED / "DATA/SPELLS.DAT"
    data = path.read_bytes()
    if len(data) != 26 * 80:
        raise ValueError(f"expected 2080-byte SPELLS.DAT, got {len(data)}")

    output: list[TierRecord] = []
    for family_id, expected_name in enumerate(FAMILY_NAMES):
        base = family_id * 80
        family_count, initial_flags = struct.unpack_from("<BB", data, base)
        if family_count != 3:
            raise ValueError(
                f"spell family {family_id} has {family_count} tiers, expected 3"
            )
        family_text_id = 160 + family_id
        observed_name = strings.get(family_text_id, expected_name)
        for tier in range(3):
            fields = SPELL_RECORD.unpack_from(data, base + 2 + tier * 26)
            primary_id, alternate_id = tier_text_ids(family_id, tier)
            tier_name = strings.get(primary_id, f"TEXT_{primary_id}")
            alternate_name = (
                strings.get(alternate_id, f"TEXT_{alternate_id}")
                if alternate_id is not None
                else ""
            )
            output.append(
                TierRecord(
                    family_id=family_id,
                    family_name=expected_name,
                    family_display_name=observed_name,
                    family_text_id=family_text_id,
                    family_header_tier_count=family_count,
                    family_initial_flags_raw=initial_flags,
                    tier=tier,
                    tier_name=tier_name,
                    tier_text_id=primary_id,
                    alternate_tier_name=alternate_name,
                    alternate_tier_text_id=(
                        str(alternate_id) if alternate_id is not None else ""
                    ),
                    effect_selector_raw=fields[0],
                    mana_cost=fields[1],
                    max_mana_requirement=fields[2],
                    single_player_experience=fields[3],
                    multiplayer_experience=fields[4],
                    hint_text_id_loaded=fields[5],
                    timing_count_raw=fields[6],
                    life_selector_raw=fields[7],
                    tier_flags_raw=fields[8],
                    family_icon_small_index=97 + family_id,
                    family_icon_big_index=123 + family_id,
                    tier_icon_index=179 + family_id * 3 + tier,
                    load_override_chain=(
                        "compiled seed -> owned DATA/SPELLS.DAT overwrite -> "
                        "SetDefaultSpells_5C0A0 -> day/night/cave patch"
                    ),
                    confidence="STRONG",
                    extraction_ready=True,
                )
            )
    return output


def compiled_spell_seed_bytes() -> bytes:
    text = (ENGINE / "Spells.cpp").read_text(encoding="utf-8")
    initializer = text.split("SPELLS_BEGIN_BUFFER_str[26] =", 1)[1]
    initializer = initializer.split("//-----", 1)[0]
    values = [int(token, 16) for token in re.findall(r"0x[0-9A-Fa-f]+", initializer)]
    expected = 26 * (2 + 3 * 9)
    if len(values) != expected:
        raise ValueError(
            f"expected {expected} compiled spell initializer values, got {len(values)}"
        )
    output = bytearray()
    cursor = 0
    for _family_id in range(26):
        output.extend(struct.pack("<BB", values[cursor], values[cursor + 1]))
        cursor += 2
        for _tier in range(3):
            output.extend(SPELL_RECORD.pack(*values[cursor:cursor + 9]))
            cursor += 9
    return bytes(output)


def spell_seed_comparison_rows(
    owned: bytes,
    seed: bytes,
) -> list[dict[str, object]]:
    field_names = (
        "effect_selector_raw",
        "mana_cost",
        "max_mana_requirement",
        "single_player_experience",
        "multiplayer_experience",
        "hint_text_id",
        "timing_count_raw",
        "life_selector_raw",
        "tier_flags_raw",
    )
    rows: list[dict[str, object]] = []
    for family_id, family_name in enumerate(FAMILY_NAMES):
        owned_header = struct.unpack_from("<BB", owned, family_id * 80)
        seed_header = struct.unpack_from("<BB", seed, family_id * 80)
        for field_name, seed_value, owned_value in zip(
            ("family_header_tier_count", "family_initial_flags_raw"),
            seed_header,
            owned_header,
            strict=True,
        ):
            if seed_value != owned_value:
                rows.append(
                    {
                        "family_id": family_id,
                        "family_name": family_name,
                        "tier": "",
                        "field": field_name,
                        "compiled_seed": seed_value,
                        "owned_table": owned_value,
                        "delta": owned_value - seed_value,
                    }
                )
        for tier in range(3):
            offset = family_id * 80 + 2 + tier * 26
            owned_fields = SPELL_RECORD.unpack_from(owned, offset)
            seed_fields = SPELL_RECORD.unpack_from(seed, offset)
            for field_name, seed_value, owned_value in zip(
                field_names,
                seed_fields,
                owned_fields,
                strict=True,
            ):
                if seed_value != owned_value:
                    rows.append(
                        {
                            "family_id": family_id,
                            "family_name": family_name,
                            "tier": tier,
                            "field": field_name,
                            "compiled_seed": seed_value,
                            "owned_table": owned_value,
                            "delta": owned_value - seed_value,
                        }
                    )
    return rows


def read_palette(path: Path) -> list[tuple[int, int, int, int]]:
    raw = path.read_bytes()
    if len(raw) != 768:
        raise ValueError(f"expected 768-byte VGA palette: {path}")
    return [
        (
            min(raw[index] * 4, 255),
            min(raw[index + 1] * 4, 255),
            min(raw[index + 2] * 4, 255),
            255,
        )
        for index in range(0, len(raw), 3)
    ]


def read_sprite_index(tab_path: Path) -> list[tuple[int, int, int]]:
    raw = tab_path.read_bytes()
    if len(raw) % 6:
        raise ValueError(f"TAB size is not divisible by six: {tab_path}")
    return [
        (offset, width, height)
        for offset, width, height in struct.iter_unpack("<IBB", raw)
    ]


def decode_sprite(
    dat: bytes,
    entry: tuple[int, int, int],
    palette: list[tuple[int, int, int, int]],
) -> Image.Image:
    offset, width, height = entry
    image = Image.new("RGBA", (max(width, 1), max(height, 1)), (0, 0, 0, 0))
    pixels = image.load()
    cursor = offset
    for y in range(height):
        x = 0
        while True:
            if cursor >= len(dat):
                raise ValueError("sprite RLE exceeds DAT buffer")
            token = dat[cursor]
            cursor += 1
            if token == 0:
                break
            if token & 0x80:
                x += 256 - token
                continue
            end = cursor + token
            if end > len(dat):
                raise ValueError("sprite pixel run exceeds DAT buffer")
            for palette_index in dat[cursor:end]:
                if 0 <= x < width:
                    pixels[x, y] = palette[palette_index]
                x += 1
            cursor = end
    return image


def save_icons(records: list[TierRecord]) -> dict[str, list[dict[str, object]]]:
    manifest: dict[str, list[dict[str, object]]] = {}
    for mode, (pack, palette_name) in MAP_MODES.items():
        dat_path = OWNED / f"DATA/{pack}.DAT"
        tab_path = OWNED / f"DATA/{pack}.TAB"
        palette_path = OWNED / f"DATA/{palette_name}"
        dat = dat_path.read_bytes()
        entries = read_sprite_index(tab_path)
        palette = read_palette(palette_path)
        mode_dir = OUTPUT / "icons" / mode
        mode_dir.mkdir(parents=True, exist_ok=True)
        mode_manifest: list[dict[str, object]] = []
        expected_icon_paths: set[Path] = set()

        for family_id, family_name in enumerate(FAMILY_NAMES):
            family_slug = slug(family_name)
            for size_name, sprite_index in (
                ("small", 97 + family_id),
                ("big", 123 + family_id),
            ):
                icon = decode_sprite(dat, entries[sprite_index], palette)
                relative = Path("icons") / mode / (
                    f"family-{family_id:02d}-{family_slug}-{size_name}.png"
                )
                icon.save(OUTPUT / relative)
                expected_icon_paths.add(OUTPUT / relative)
                mode_manifest.append(
                    {
                        "namespace": "asset",
                        "raw_key": f"{pack}:{sprite_index}",
                        "mode": mode,
                        "family_id": family_id,
                        "tier": None,
                        "display_name": family_name,
                        "text_id": 160 + family_id,
                        "role": f"family_{size_name}_icon",
                        "path": str(relative),
                        "width": icon.width,
                        "height": icon.height,
                    }
                )

        for record in records:
            icon = decode_sprite(dat, entries[record.tier_icon_index], palette)
            display_name = (
                record.alternate_tier_name
                if mode != "day" and record.alternate_tier_name
                else record.tier_name
            )
            text_id = (
                int(record.alternate_tier_text_id)
                if mode != "day" and record.alternate_tier_text_id
                else record.tier_text_id
            )
            relative = Path("icons") / mode / (
                f"tier-{record.family_id:02d}-{record.tier}-"
                f"{slug(display_name)}.png"
            )
            icon.save(OUTPUT / relative)
            expected_icon_paths.add(OUTPUT / relative)
            mode_manifest.append(
                {
                    "namespace": "asset",
                    "raw_key": f"{pack}:{record.tier_icon_index}",
                    "mode": mode,
                    "family_id": record.family_id,
                    "tier": record.tier,
                    "display_name": display_name,
                    "text_id": text_id,
                    "role": "tier_icon",
                    "path": str(relative),
                    "width": icon.width,
                    "height": icon.height,
                }
            )
        for existing in mode_dir.glob("*.png"):
            if existing not in expected_icon_paths:
                existing.unlink()
        manifest[mode] = mode_manifest
        save_contact_sheet(mode, mode_manifest)
    return manifest


def save_contact_sheet(mode: str, entries: list[dict[str, object]]) -> None:
    selected = [
        entry
        for entry in entries
        if entry["role"] in {"family_big_icon", "tier_icon"}
    ]
    columns = 6
    cell_width = 180
    cell_height = 116
    rows = (len(selected) + columns - 1) // columns
    sheet = Image.new(
        "RGBA",
        (columns * cell_width, rows * cell_height),
        (18, 18, 24, 255),
    )
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.load_default()
    for index, entry in enumerate(selected):
        column = index % columns
        row = index // columns
        x0 = column * cell_width
        y0 = row * cell_height
        icon = Image.open(OUTPUT / str(entry["path"])).convert("RGBA")
        scale = min(72 / max(icon.width, 1), 72 / max(icon.height, 1), 4)
        if scale != 1:
            icon = icon.resize(
                (
                    max(1, round(icon.width * scale)),
                    max(1, round(icon.height * scale)),
                ),
                Image.Resampling.NEAREST,
            )
        sheet.alpha_composite(
            icon,
            (
                x0 + (cell_width - icon.width) // 2,
                y0 + 4 + (72 - icon.height) // 2,
            ),
        )
        family_id = int(entry["family_id"])
        tier = entry["tier"]
        label = (
            f"{family_id:02d} {entry['display_name']}"
            if tier is None
            else f"{family_id:02d}.{tier} {entry['display_name']}"
        )
        draw.text((x0 + 4, y0 + 82), label[:28], fill=(235, 235, 240, 255), font=font)
        draw.text(
            (x0 + 4, y0 + 96),
            str(entry["raw_key"]),
            fill=(155, 170, 190, 255),
            font=font,
        )
    sheet.save(OUTPUT / f"spell-icons-{mode}-contact.png")


def save_owned_sprite_atlases() -> list[dict[str, object]]:
    """Decode every owned HSPR/MSPR row into bounded review contact sheets."""
    output_rows: list[dict[str, object]] = []
    columns = 8
    rows_per_page = 8
    per_page = columns * rows_per_page
    cell_width = 112
    cell_height = 104
    atlas_root = OUTPUT / "sprite-atlas"
    atlas_root.mkdir(parents=True, exist_ok=True)

    for mode, (_, palette_name) in MAP_MODES.items():
        palette = read_palette(OWNED / f"DATA/{palette_name}")
        mode_code = {"day": "D", "night": "N", "cave": "C"}[mode]
        for pack_kind in ("HSPR", "MSPR"):
            pack = f"{pack_kind}{mode_code}0-0"
            dat = (OWNED / f"DATA/{pack}.DAT").read_bytes()
            entries = read_sprite_index(OWNED / f"DATA/{pack}.TAB")
            pack_dir = atlas_root / mode / pack_kind.casefold()
            pack_dir.mkdir(parents=True, exist_ok=True)
            expected_pages: set[Path] = set()
            decoded: list[Image.Image] = []
            for sprite_index, entry in enumerate(entries):
                image = decode_sprite(dat, entry, palette)
                decoded.append(image)
                page = sprite_index // per_page
                cell = sprite_index % per_page
                output_rows.append(
                    {
                        "namespace": "asset",
                        "raw_key": f"{pack}:{sprite_index}",
                        "mode": mode,
                        "pack_kind": pack_kind,
                        "sprite_index": sprite_index,
                        "width": entry[1],
                        "height": entry[2],
                        "dat_offset": entry[0],
                        "drawable": bool(entry[1] and entry[2]),
                        "contact_sheet": str(
                            Path("sprite-atlas")
                            / mode
                            / pack_kind.casefold()
                            / f"page-{page:02d}.png"
                        ),
                        "contact_cell": cell,
                        "semantic_label": "",
                        "guardrail": (
                            "visual identity is evidence only when joined to a "
                            "causally consumed sprite or particle index"
                        ),
                    }
                )
            for page_start in range(0, len(decoded), per_page):
                page_number = page_start // per_page
                page_path = pack_dir / f"page-{page_number:02d}.png"
                expected_pages.add(page_path)
                sheet = Image.new(
                    "RGBA",
                    (columns * cell_width, rows_per_page * cell_height),
                    (18, 18, 24, 255),
                )
                draw = ImageDraw.Draw(sheet)
                font = ImageFont.load_default()
                for local_index, image in enumerate(
                    decoded[page_start : page_start + per_page]
                ):
                    sprite_index = page_start + local_index
                    thumbnail = image.copy()
                    thumbnail.thumbnail((88, 78), Image.Resampling.NEAREST)
                    column = local_index % columns
                    row = local_index // columns
                    x0 = column * cell_width
                    y0 = row * cell_height
                    sheet.alpha_composite(
                        thumbnail,
                        (
                            x0 + (cell_width - thumbnail.width) // 2,
                            y0 + 4 + (78 - thumbnail.height) // 2,
                        ),
                    )
                    draw.text(
                        (x0 + 4, y0 + 84),
                        f"{sprite_index:03d} {image.width}x{image.height}",
                        fill=(220, 225, 235, 255),
                        font=font,
                    )
                sheet.save(page_path)
            for existing in pack_dir.glob("page-*.png"):
                if existing not in expected_pages:
                    existing.unlink()
    return output_rows


def indexed_raw_image(
    raw_path: Path,
    width: int,
    height: int,
    palette: list[tuple[int, int, int, int]],
) -> Image.Image:
    raw = raw_path.read_bytes()
    if len(raw) != width * height:
        raise ValueError(
            f"indexed image size mismatch for {raw_path}: "
            f"{len(raw)} != {width}*{height}"
        )
    image = Image.frombytes("P", (width, height), raw)
    image.putpalette([channel for color in palette for channel in color[:3]])
    return image.convert("RGBA")


def save_tmap_evidence() -> list[dict[str, object]]:
    """Render the exact private visual correlations used by semantic claims."""
    evidence_dir = OUTPUT / "evidence"
    evidence_dir.mkdir(parents=True, exist_ok=True)

    mushroom_source = HIGH_RES / "TMAPS/TMAPS2-2-424-00.data"
    mushroom = indexed_raw_image(
        mushroom_source,
        208,
        172,
        read_palette(OWNED / "DATA/PALC-0.DAT"),
    )
    mushroom_relative = Path("evidence/tmap-cave-424-mushroom.png")
    mushroom.save(OUTPUT / mushroom_relative)

    terrain_source = HIGH_RES / "block128.data"
    terrain_stream = indexed_raw_image(
        terrain_source,
        256,
        9728,
        read_palette(OWNED / "DATA/PALD-0.DAT"),
    )
    sheet = Image.new("RGBA", (3 * 136, 154), (18, 18, 24, 255))
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.load_default()
    for column, terrain_id in enumerate((10, 11, 12)):
        x = (terrain_id % 2) * 128
        y = (terrain_id // 2) * 128
        tile = terrain_stream.crop((x, y, x + 128, y + 128))
        sheet.alpha_composite(tile, (column * 136 + 4, 4))
        draw.text(
            (column * 136 + 4, 136),
            f"mapTerrainType {terrain_id}",
            fill=(230, 230, 235, 255),
            font=font,
        )
    lava_relative = Path("evidence/terrain-lava-family-10-12.png")
    sheet.save(OUTPUT / lava_relative)
    return [
        {
            "claim_id": "runtime-cave-scenery-6",
            "raw_key": "TMAPS2-2:424",
            "semantic_label": "cave mushroom visual",
            "source_path": str(mushroom_source.relative_to(LAB_ROOT)),
            "source_sha256": sha256(mushroom_source),
            "palette_path": str(
                (OWNED / "DATA/PALC-0.DAT").relative_to(LAB_ROOT)
            ),
            "rendered_path": str(mushroom_relative),
            "guardrail": "visual identity is joined through particle records 324..327",
        },
        {
            "claim_id": "terrain-lava-material-family",
            "raw_key": "mapTerrainType=10,11,12",
            "semantic_label": "three-member lava terrain-material family",
            "source_path": str(terrain_source.relative_to(LAB_ROOT)),
            "source_sha256": sha256(terrain_source),
            "palette_path": str(
                (OWNED / "DATA/PALD-0.DAT").relative_to(LAB_ROOT)
            ),
            "rendered_path": str(lava_relative),
            "guardrail": "individual hot, crust, or animation-state names remain unresolved",
        },
    ]


def write_csv(path: Path, rows: list[dict[str, object]]) -> None:
    if not rows:
        raise ValueError(f"refusing to write empty CSV: {path}")
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def runtime_class_rows() -> list[dict[str, object]]:
    table_lines = {
        0: "2060-2077",
        1: "1146-1160",
        2: "1162-1199",
        3: "1201-1231",
        4: "1234-1240",
        5: "1242-1512",
        6: "1514-1530",
        7: "1514-1530",
        8: "1514-1530",
        9: "1532-1599",
        10: "1601-1796",
        11: "1798-1893",
        12: "1894-1899",
        13: "1900-1924",
        14: "1926-1947",
        15: "1949-2059",
    }
    return [
        {
            "runtime_class": class_id,
            "semantic_family": label,
            "confidence": confidence,
            "identity_basis": basis,
            "notes": notes,
            "dispatch_table_anchor": f"EventsFunctions.cpp:{table_lines[class_id]}",
            "class_registry_anchor": "EventsFunctions.cpp:2060-2077",
            "constructor_dispatch_anchor": "Events.cpp:5186-5190",
            "action_dispatch_anchor": "EventsFunctions.cpp:40124-40187",
        }
        for class_id, label, confidence, basis, notes in RUNTIME_CLASSES
    ]


def creature_model_rows() -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    for (
        model,
        label,
        enabled,
        initial_action,
        fast_cap,
        slow_floor,
        life,
        mana,
        constructor,
        definition_lines,
        confidence,
        identity_basis,
        notes,
    ) in CREATURE_MODELS:
        action_start = model * 8
        action_end = action_start + 7
        action_table_start = 1243 + model * 8
        action_table_end = action_table_start + 7
        anchors = (
            f"EventsFunctions.cpp:{action_table_start}-{action_table_end}; "
            f"EventsFunctions.cpp:{1482 + model}; "
            f"EventsFunctions.cpp:{definition_lines}"
        )
        if model in CREATURE_EXTRA_ANCHORS:
            anchors += f"; {CREATURE_EXTRA_ANCHORS[model]}"
        rows.append(
            {
                "runtime_class": 5,
                "runtime_model": model,
                "semantic_label": label,
                "constructor_enabled": enabled,
                "regular_action_start": action_start,
                "regular_action_end": action_end,
                "constructor_function": constructor,
                "stored_constructor_address": CREATURE_CONSTRUCTOR_ADDRESSES[model],
                "initial_action": initial_action,
                "fast_cap_raw_minSpeed": fast_cap,
                "slow_floor_raw_maxSpeed": slow_floor,
                "initial_life": life,
                "mana_or_drop_parameters": mana,
                "confidence": confidence,
                "identity_basis": identity_basis,
                "notes": notes,
                "evidence_anchors": anchors,
            }
        )
    return rows


def creature_action_exception_rows() -> list[dict[str, object]]:
    return [
        {
            "action_index": 232,
            "status": "enabled",
            "group_relation": "separate multipart segment group token 0x002A5C38",
            "stored_address": "0x001FC6B0",
            "runtime_observation": "outside all regular eight-action model bands",
            "evidence_anchors": "EventsFunctions.cpp:1475",
            "risk": "requires semantic identity and representative runtime trace",
        },
        {
            "action_index": 233,
            "status": "disabled null dispatch row",
            "group_relation": "uses model-27 Hydra group token 0x002A5C30",
            "stored_address": "",
            "runtime_observation": "Hydra constructor writes this action to branch/part entities",
            "evidence_anchors": "EventsFunctions.cpp:1476; EventsFunctions.cpp:34599-34701",
            "risk": "material reconstructed-source/runtime inconsistency",
        },
        {
            "action_index": 234,
            "status": "disabled null dispatch row",
            "group_relation": "uses model-27 Hydra group token 0x002A5C30",
            "stored_address": "",
            "runtime_observation": "Hydra constructor writes this action to branch/part entities",
            "evidence_anchors": "EventsFunctions.cpp:1477; EventsFunctions.cpp:34599-34701",
            "risk": "material reconstructed-source/runtime inconsistency",
        },
    ]


def runtime_model_annotation_rows(
    family_rows: list[dict[str, object]],
) -> list[dict[str, object]]:
    """Friendly overlays for constructor physical slots, preserving selectors."""
    rows: list[dict[str, object]] = []

    def add(
        runtime_class: int,
        model: int,
        label: str,
        confidence: str,
        identity_basis: str,
        notes: str,
        source_line: int,
    ) -> None:
        rows.append(
            {
                "runtime_class": runtime_class,
                "constructor_physical_slot": model,
                "semantic_label": label,
                "confidence": confidence,
                "identity_basis": identity_basis,
                "notes": notes,
                "constructor_table_anchor": f"EventsFunctions.cpp:{source_line}",
            }
        )

    class2 = (
        ("Tree", "CONFIRMED", "named constructor and owned-level/editor correlation", ""),
        ("Standing stone", "STRONG", "named constructor and editor correlation", ""),
        ("Dolmen or stonehenge", "STRONG", "named constructor and editor correlation", ""),
        ("Bad stone", "STRONG", "constructor plus editor correlation", ""),
        ("scenery model 02:04", "PROVISIONAL", "constructor behavior only", "friendly identity unresolved"),
        ("scenery model 02:05", "PROVISIONAL", "constructor behavior only", "particle parameter has no drawable TMAP identity"),
        ("cave mushroom scenery", "CONFIRMED", "constructor -> particle 324..327 -> cave TMAP 424", "recovered Bee comment is contradicted by the consumed artwork"),
        ("Barrel", "STRONG", "constructor plus editor correlation", ""),
        ("Vase", "STRONG", "constructor plus editor correlation", ""),
    )
    for model, (label, confidence, basis, notes) in enumerate(class2):
        add(2, model, label, confidence, basis, notes, 1189 + model)

    class3 = (
        ("primary player wizard/carpet controller", "STRONG", "named AddPlayer constructor", ""),
        ("alternate or opponent player controller", "STRONG", "constructor behavior", ""),
        ("castle root entity", "STRONG", "constructor behavior and castle consumers", ""),
        ("balloon entity", "STRONG", "constructor behavior and balloon consumers", ""),
    )
    for model, (label, confidence, basis, notes) in enumerate(class3):
        add(3, model, label, confidence, basis, notes, 1219 + model)
    for model in range(4, 12):
        add(
            3,
            model,
            f"global player-position marker writer {model - 4}",
            "STRONG",
            "constructor writes array_0x2362 and returns null",
            "constructor selector is not a live runtime entity model",
            1219 + model,
        )

    class9_labels = (
        "ordinary Fireball projectile",
        "Possession projectile",
        "Earthquake delivery entity",
        "Meteor projectile",
        "Volcano delivery entity",
        "Crater delivery entity",
        "unresolved spell projectile 9:06",
        "Duel delivery entity",
        "Steal Mana delivery entity",
        "standard Lightning delivery entity",
        "Castle placement delivery entity",
        "unresolved spell projectile 9:11",
        "enhanced Lightning delivery entity",
        "unresolved AI projectile 9:13",
        "unresolved slow projectile 9:14",
        "unresolved short-lived projectile 9:15",
        "enabled missing reconstructed constructor 9:16",
        "enhanced Possession delivery entity",
        "enabled missing reconstructed constructor 9:18",
        "enabled missing reconstructed constructor 9:19",
        "AI attack projectile 9:20",
        "AI attack projectile 9:21",
        "Gravity Well delivery entity",
        "Tremor delivery entity",
        "Summon Army delivery entity",
        "Alliance delivery entity",
        "Whirlwind delivery entity",
        "unresolved spell projectile 9:27",
        "enhanced Fireball or Fire Storm projectile",
        "Magic Mine delivery entity",
        "Cave In delivery entity",
    )
    provisional9 = {6, 11, 13, 14, 15, 16, 18, 19, 20, 21, 27}
    for model, label in enumerate(class9_labels):
        add(
            9,
            model,
            label,
            "PROVISIONAL" if model in provisional9 else "STRONG",
            "constructor/action behavior and spell casting consumers",
            (
                "friendly role or missing reconstruction remains unresolved"
                if model in provisional9
                else ""
            ),
            1568 + model,
        )

    class10_known = {
        0: ("ordinary Fireball impact/explosion", "STRONG", "ordinary projectile consumer"),
        1: ("large explosion or death effect", "STRONG", "constructor and death consumers"),
        5: ("water splash effect", "STRONG", "projectile water-impact consumer"),
        8: ("explicit no-op mini-volcano slot", "STRONG", "constructor returns null"),
        9: ("volcano terrain effect", "STRONG", "Volcano spell consumer"),
        11: ("crater terrain effect", "STRONG", "Crater spell consumer"),
        12: ("Possession-related impact", "STRONG", "Possession consumer"),
        13: ("smoke particle 3B", "STRONG", "named constructor"),
        14: ("smoke particle 3C", "STRONG", "named constructor"),
        15: ("Earthquake impact", "STRONG", "Earthquake spell consumer"),
        17: ("Meteor impact", "STRONG", "Meteor spell consumer"),
        20: ("enabled missing reconstructed constructor 10:20", "EXACT_STATIC", "table/dispatcher audit"),
        21: ("enabled missing reconstructed constructor 10:21", "EXACT_STATIC", "table/dispatcher audit"),
        22: ("wind vortex or whirlwind impact", "STRONG", "named AddWind constructor and spell consumer"),
        23: ("Lightning impact", "STRONG", "Lightning spell consumer"),
        24: ("enabled missing reconstructed constructor 10:24", "EXACT_STATIC", "table/dispatcher audit"),
        25: ("Steal Mana impact", "STRONG", "Steal Mana spell consumer"),
        26: ("Duel impact", "STRONG", "Duel spell consumer"),
        29: ("path/topology terrain command", "STRONG", "terrain-command consumer"),
        30: ("path point and black-smoke effect", "STRONG", "named constructor"),
        31: ("canyon or tunnel terrain command", "STRONG", "terrain-command consumer"),
        35: ("bare allocator result without class/model initialization", "EXACT_STATIC", "constructor body"),
        39: ("mana sphere model 39", "STRONG", "named mana-sphere constructor"),
        43: ("castle-related impact or target effect", "STRONG", "castle consumer"),
        45: ("building terrain-modification command", "STRONG", "named constructor and BUILD consumer"),
        54: ("auxiliary effect model 54", "STRONG", "named constructor"),
        57: ("Summon Army-related effect", "STRONG", "Summon Army consumer"),
        58: ("mana-sphere constructor alias producing model 39", "EXACT_STATIC", "constructor body"),
        59: ("checkpoint-arrival effect", "STRONG", "named constructor"),
        60: ("smoke effect", "STRONG", "named constructor"),
        67: ("Gravity Well impact", "STRONG", "Gravity Well spell consumer"),
        69: ("auxiliary-constructor alias producing model 54", "EXACT_STATIC", "constructor body"),
        71: ("Tremor impact", "STRONG", "Tremor spell consumer"),
        72: ("Summon Army projectile impact", "STRONG", "Summon Army consumer"),
        74: ("Alliance projectile impact", "STRONG", "Alliance consumer"),
        76: ("enhanced Fireball sphere root", "STRONG", "Fire Storm consumer and multipart constructor"),
        77: ("disabled factory slot; manually created sphere part", "EXACT_STATIC", "model-76 constructor body"),
        78: ("Magic Mine impact", "STRONG", "Magic Mine consumer"),
        79: ("castle-defense entity", "STRONG", "named constructor"),
        88: ("enabled missing reconstructed constructor 10:88", "EXACT_STATIC", "table/dispatcher audit"),
        89: ("Cave In impact", "STRONG", "Cave In spell consumer"),
        90: ("enabled missing reconstructed constructor 10:90", "EXACT_STATIC", "table/dispatcher audit"),
    }
    disabled10 = {37, 46, 47, 48, 49, 75, 77}
    for model in range(92):
        if model in class10_known:
            label, confidence, basis = class10_known[model]
            notes = ""
        elif model in disabled10:
            label = f"disabled class-10 constructor slot {model}"
            confidence = "EXACT_STATIC"
            basis = "zero or disabled constructor-table row"
            notes = "not generically constructible through this table"
        else:
            label = f"class-10 effect model {model}; friendly identity unresolved"
            confidence = "PROVISIONAL"
            basis = "constructor function and behavior only"
            notes = "retain raw class/model key"
        add(10, model, label, confidence, basis, notes, 1704 + model)

    for model in range(45):
        add(
            11,
            model,
            f"switch, trigger, or checkpoint raw subtype {model}",
            "STRONG",
            "constructor writes class 11 and the same raw subtype",
            (
                "enabled row lacks a reconstructed constructor/dispatcher case"
                if model in range(5, 12)
                else "friendly trigger semantics unresolved"
            ),
            1847 + model,
        )

    class14 = (
        ("generic special fixture 14:00", "PROVISIONAL", "shared actions and sprite 77", "friendly identity unresolved"),
        ("special terrain or world controller 14:01", "PROVISIONAL", "constructor and unique action", "friendly identity unresolved"),
        ("cave-only special controller 14:02", "PROVISIONAL", "cave-gated constructor and unique action", "friendly identity unresolved"),
        ("special fixture using sprite 338", "PROVISIONAL", "unique constructor sprite", "friendly identity unresolved"),
        ("special fixture using sprite 339", "PROVISIONAL", "unique constructor sprite", "friendly identity unresolved"),
        ("collectible spell scroll", "STRONG", "named UpdateScroll action and constructor behavior", ""),
    )
    for model, (label, confidence, basis, notes) in enumerate(class14):
        add(14, model, label, confidence, basis, notes, 1941 + model)

    for creature in creature_model_rows():
        add(
            5,
            int(creature["runtime_model"]),
            str(creature["semantic_label"]),
            str(creature["confidence"]),
            str(creature["identity_basis"]),
            str(creature["notes"]),
            1482 + int(creature["runtime_model"]),
        )
    for family in family_rows:
        model = int(family["family_id"])
        add(
            15,
            model,
            f"{family['family_name']} spell holder",
            str(family["confidence"]),
            "authoritative spell enum and class-15 constructor band",
            "",
            2032 + model,
        )

    rows.sort(
        key=lambda row: (
            int(row["runtime_class"]),
            int(row["constructor_physical_slot"]),
        )
    )
    return rows


def runtime_action_annotation_rows() -> list[dict[str, object]]:
    """Per-physical-row action semantics for non-banded runtime classes.

    Physical slot is deliberately the identity key.  In particular, class 9
    constructors stop matching action indices after model 13, and class 10
    physical action slot 63 stores selector 41.  Neither anomaly may be
    normalized away by joining on a guessed model/action equivalence.
    """
    rows: list[dict[str, object]] = []

    def add(
        runtime_class: int,
        slot: int,
        label: str,
        behavior: str,
        confidence: str,
        linkage: str,
        evidence: str,
        annotation_level: str = "INDIVIDUAL_BEHAVIOR",
        notes: str = "",
    ) -> None:
        rows.append(
            {
                "runtime_class": runtime_class,
                "action_physical_slot": slot,
                "semantic_label": label,
                "behavior_summary": behavior,
                "confidence": confidence,
                "constructor_or_transition_linkage": linkage,
                "evidence_anchors": evidence,
                "annotation_level": annotation_level,
                "notes": notes,
            }
        )

    class2 = (
        ("Tree live update", "terrain snap; pending damage; destruction result; water splash/removal", "STRONG", "model 0 initial", "EventsFunctions.cpp:62447-62505"),
        ("Tree dying phase", "life countdown; sprite transition; terrain snap; water removal", "STRONG", "model 0 transition", "EventsFunctions.cpp:62510-62544"),
        ("Tree terminal phase", "terrain snap and water splash/removal", "STRONG", "model 0 transition", "EventsFunctions.cpp:62548-62563"),
        ("standing-stone static update", "sets static flag and snaps to terrain", "STRONG", "model 1 initial", "EventsFunctions.cpp:62566-62570"),
        ("missing scenery action 02:04", "enabled stored address absent from reconstructed dispatcher", "EXACT_STATIC", "model 1 secondary row", "EventsFunctions.cpp:1167"),
        ("missing scenery action 02:05", "enabled stored address absent from reconstructed dispatcher", "EXACT_STATIC", "model 1 secondary row", "EventsFunctions.cpp:1168"),
        ("dolmen regeneration-field update", "nearby live players receive the high-regeneration flag; terrain snap", "STRONG", "model 2 initial", "EventsFunctions.cpp:62573-62582"),
        ("missing scenery action 02:07", "enabled stored address absent from reconstructed dispatcher", "EXACT_STATIC", "model 2 secondary row", "EventsFunctions.cpp:1170"),
        ("missing scenery action 02:08", "enabled stored address absent from reconstructed dispatcher", "EXACT_STATIC", "model 2 secondary row", "EventsFunctions.cpp:1171"),
        ("bad-stone static update", "sets static flag and snaps to terrain", "STRONG", "model 3 initial", "EventsFunctions.cpp:62585-62593"),
        ("explicit inert scenery action 02:10", "dispatcher deliberately performs no call", "EXACT_STATIC", "no proven constructor", "Events.cpp:3199-3207"),
        ("explicit inert scenery action 02:11", "dispatcher deliberately performs no call", "EXACT_STATIC", "no proven constructor", "Events.cpp:3199-3207"),
        ("explicit inert scenery action 02:12", "dispatcher deliberately performs no call", "EXACT_STATIC", "model 4 initial", "Events.cpp:3199-3207"),
        ("explicit inert scenery action 02:13", "dispatcher deliberately performs no call", "EXACT_STATIC", "no proven constructor", "Events.cpp:3199-3207"),
        ("explicit inert scenery action 02:14", "dispatcher deliberately performs no call", "EXACT_STATIC", "no proven constructor", "Events.cpp:3199-3207"),
        ("explicit inert scenery action 02:15", "dispatcher deliberately performs no call", "EXACT_STATIC", "model 5 initial", "Events.cpp:3199-3207"),
        ("explicit inert scenery action 02:16", "dispatcher deliberately performs no call", "EXACT_STATIC", "no proven constructor", "Events.cpp:3199-3207"),
        ("explicit inert scenery action 02:17", "dispatcher deliberately performs no call", "EXACT_STATIC", "no proven constructor", "Events.cpp:3199-3207"),
        ("cave ambient live update", "pending damage; death transition/result; terrain snap; water removal", "STRONG", "model 6 initial", "EventsFunctions.cpp:62596-62627"),
        ("cave ambient terminal update", "terrain snap and water removal", "STRONG", "model 6 transition", "EventsFunctions.cpp:62630-62639"),
        ("barrel ballistic/bounce update", "horizontal motion; vertical acceleration clamp; bounce; damage; water/death effects", "STRONG", "model 7 initial", "EventsFunctions.cpp:62648-62749"),
        ("vase ballistic/bounce update", "same shared ballistic, bounce, damage, water, and death behavior as action 20", "STRONG", "model 8 initial", "EventsFunctions.cpp:62642-62749"),
    )
    for slot, values in enumerate(class2):
        add(2, slot, *values)

    class3 = (
        ("primary player simulation", "flight/movement; effect transfer; mana/life regeneration; death transition", "STRONG", "model 0 initial", "EventsFunctions.cpp:60003-60090"),
        ("opponent wizard AI simulation", "AI state machine; cooldowns; castle proximity; regeneration; targeting and altitude clamps", "STRONG", "model 1 initial", "EventsFunctions.cpp:5243-5315"),
        ("player death phase", "fall; score and notification; release spell holders; create replacement; countdown", "STRONG", "transition from player simulation", "EventsFunctions.cpp:60093-60230"),
        ("player banished or respawn phase", "AI respawn countdown or banished notification; human camera/pose update", "STRONG", "transition from death", "EventsFunctions.cpp:60302-60356"),
        ("castle settling and absorption", "castle completion; state transition; ground snap; absorb nearby owned mana", "STRONG", "castle transition", "EventsFunctions.cpp:61103-61168"),
        ("castle creation state machine", "placement validation; construction helpers; color and geometry setup", "STRONG", "model 2 initial", "EventsFunctions.cpp:61171-61228"),
        ("castle destruction state", "teardown; restore or expel mana; return to settling state", "STRONG", "castle transition", "EventsFunctions.cpp:61270-61286"),
        ("explicit inert balloon initial state", "dispatcher deliberately performs no call", "EXACT_STATIC", "model 3 initial", "Events.cpp:2997-2999"),
        ("explicit inert balloon state 8", "dispatcher deliberately performs no call", "EXACT_STATIC", "no proven transition", "Events.cpp:3000-3002"),
        ("active balloon update", "target following; mana/heal collection and ownership transfer; terrain/cave movement", "STRONG", "model 3 transition", "EventsFunctions.cpp:61811-61919"),
        ("explicit inert class-3 state 10", "dispatcher deliberately performs no call", "EXACT_STATIC", "no proven transition", "Events.cpp:3012-3014"),
        ("end-game sequence variant 11", "shared staged camera, light, audio, and data sequence", "STRONG", "player end sequence", "EventsFunctions.cpp:60361-60644"),
        ("end-game sequence variant 12", "same shared sequence; selector chooses the global end actor", "STRONG", "player end sequence", "EventsFunctions.cpp:60361-60644"),
    )
    for slot, values in enumerate(class3):
        add(3, slot, *values)

    class9 = (
        ("ordinary Fireball carrier", "projectile flight; entity, terrain, cave, and water collision; configured impact", "STRONG", "model 0", "EventsFunctions.cpp:63053-63058"),
        ("Possession carrier", "guided flight; collision; configured impact; hit and experience bookkeeping", "STRONG", "model 1", "EventsFunctions.cpp:63258-63375"),
        ("Earthquake carrier", "generic carrier resolver; copies payload into spawned effect life", "STRONG", "model 2", "EventsFunctions.cpp:63377-63386"),
        ("Meteor carrier", "generic resolver; extra fiery visual; copies payload into impact life", "STRONG", "model 3", "EventsFunctions.cpp:63388-63426"),
        ("Volcano carrier", "generic resolver; copies payload into impact max-life", "STRONG", "model 4", "EventsFunctions.cpp:63428-63442"),
        ("Crater carrier", "generic resolver; copies payload into impact life", "STRONG", "model 5", "EventsFunctions.cpp:63444-63453"),
        ("unresolved spell carrier 09:06", "plain generic flight and collision resolver", "PROVISIONAL", "model 6", "EventsFunctions.cpp:63455-63459"),
        ("Duel carrier", "guided damaging projectile path shared with Steal Mana", "STRONG", "model 7", "EventsFunctions.cpp:63461-63619"),
        ("Steal Mana carrier", "guided damaging projectile path shared with Duel", "STRONG", "model 8", "EventsFunctions.cpp:63467-63619"),
        ("Lightning path carrier", "stepwise path; creates action-14 bolt segments; terminal impact and bookkeeping", "STRONG", "model 9", "EventsFunctions.cpp:58316-58507"),
        ("Castle-placement carrier", "guided placement; terrain/cave/arrival resolution; create or refund", "STRONG", "model 10", "EventsFunctions.cpp:58509-58605"),
        ("unresolved spell carrier 09:11", "plain generic flight and collision resolver", "PROVISIONAL", "model 11", "EventsFunctions.cpp:58733-58737"),
        ("enhanced Lightning carrier", "guided collision; fixed impact subtype; hit bookkeeping", "STRONG", "model 12", "EventsFunctions.cpp:58739-58898"),
        ("arrow carrier", "flight; launch sound; collision and pending-damage submission", "STRONG", "model 13", "EventsFunctions.cpp:58900-58952"),
        ("Lightning segment lifetime", "short lifetime countdown; internally created by action 9", "STRONG", "orphan physical action; no constructor", "EventsFunctions.cpp:58381-58393; EventsFunctions.cpp:58954-58964"),
        ("unresolved slow carrier 09:15", "plain generic resolver; constructor uses slow speed 128", "PROVISIONAL", "model 14", "EventsFunctions.cpp:58966-58970"),
        ("unresolved short-lived carrier 09:16", "plain generic resolver; constructor uses max-life 80 and speed 128", "PROVISIONAL", "model 15", "EventsFunctions.cpp:58972-58976"),
        ("orphan carrier action 09:17", "generic resolver plus owned fire visual for live carrier", "PROVISIONAL", "missing constructor slot 16 does not prove linkage", "EventsFunctions.cpp:58978-58998"),
        ("enhanced Possession carrier", "guided collision; creates possession impact variants; propagates payload", "STRONG", "model 17", "EventsFunctions.cpp:59000-59115"),
        ("orphan immediate-impact carrier 09:19", "creates configured effect immediately; copies owner/orientation/subspell", "PROVISIONAL", "missing constructor slot 18 does not prove linkage", "EventsFunctions.cpp:59117-59132"),
        ("orphan generic carrier 09:20", "plain generic flight and collision resolver", "PROVISIONAL", "missing constructor slot 19 does not prove linkage", "EventsFunctions.cpp:59134-59138"),
        ("AI attack carrier 09:21", "plain generic flight and collision resolver", "PROVISIONAL", "model 20", "EventsFunctions.cpp:59140-59144"),
        ("AI attack carrier 09:22", "plain generic flight and collision resolver", "PROVISIONAL", "model 21", "EventsFunctions.cpp:59146-59150"),
        ("Gravity Well carrier", "generic resolver; copies payload into spawned life", "STRONG", "model 22", "EventsFunctions.cpp:59152-59166"),
        ("Tremor carrier", "generic resolver; payload high nibble becomes spawned life", "STRONG", "model 23", "EventsFunctions.cpp:59168-59184"),
        ("Summon Army carrier", "selects actor payload; generic resolver; propagates owner through spawned chain", "STRONG", "model 24", "EventsFunctions.cpp:59186-59227"),
        ("Alliance carrier", "generic resolver; clears owning wizard target field", "STRONG", "model 25", "EventsFunctions.cpp:59229-59248"),
        ("Whirlwind carrier", "generic resolver; spawned lifetime is eight times payload", "STRONG", "model 26", "EventsFunctions.cpp:59250-59264"),
        ("unresolved spell carrier 09:28", "plain generic flight and collision resolver", "PROVISIONAL", "model 27", "EventsFunctions.cpp:59282-59286"),
        ("enhanced Fireball or Fire Storm carrier", "ordinary Fireball resolver plus multipart impact-chain ownership propagation", "STRONG", "model 28", "EventsFunctions.cpp:63060-63104"),
        ("Magic Mine carrier", "seeks eligible mine target; homes and arms target; otherwise resolves expiry/collision", "STRONG", "model 29", "EventsFunctions.cpp:59288-59420"),
        ("Cave In carrier", "generic resolver; copies payload into impact max-life", "STRONG", "model 30", "EventsFunctions.cpp:59266-59280"),
    )
    for slot, values in enumerate(class9):
        add(9, slot, *values)

    class10 = (
        ("ordinary Fireball impact", "once-only pending damage; material marks; sound; terrain/cave clamp; expiry", "STRONG", "model 0 initial", "EventsFunctions.cpp:22692-22767"),
        ("large explosion or death effect", "sound; expanding mask; spawns model-0 impact particles", "STRONG", "model 1 initial", "EventsFunctions.cpp:22768-22813"),
        ("Speed spell timed effect", "lifetime countdown and despawn only", "EXACT_STATIC", "model 2 initial", "EventsFunctions.cpp:22814-22825"),
        ("timed effect 10:03", "lifetime countdown and despawn only", "EXACT_STATIC", "model 3 initial", "EventsFunctions.cpp:23162-23168"),
        ("explicit inert effect 10:04", "enabled dispatcher case deliberately performs no call", "EXACT_STATIC", "model 4 initial", "Events.cpp:2365-2367"),
        ("water splash or ash effect", "generic effect motion; sound 27; expiry", "STRONG", "model 5 initial", "EventsFunctions.cpp:23169-23186"),
        ("ground fire effect", "animation; smoke; damage; terrain following; water extinguish", "STRONG", "model 6 initial", "EventsFunctions.cpp:23099-23155"),
        ("immediate-despawn effect 10:07", "immediate despawn", "EXACT_STATIC", "model 7 initial", "EventsFunctions.cpp:23156-23161"),
        ("immediate-despawn effect 10:08", "immediate despawn; model-8 constructor itself is inert", "EXACT_STATIC", "no live constructor", "EventsFunctions.cpp:23187-23192"),
        ("Volcano terrain-growth state machine", "raises and shapes terrain/shading; area damage; sound; secondary effects", "STRONG", "model 9 initial", "EventsFunctions.cpp:23193-23441"),
        ("one-shot material impulse", "randomized terrain/material impulse then despawn", "PROVISIONAL", "model 10 initial", "EventsFunctions.cpp:23442-23459"),
        ("Crater terrain-damage pulse", "expanding hits; terrain depression; sound; expiry", "STRONG", "model 11 initial", "EventsFunctions.cpp:23490-23545"),
        ("Possession impact variant 0", "generic effect update plus possession result variant zero", "STRONG", "model 12 initial", "EventsFunctions.cpp:23546-23558"),
        ("smoke particle 3B", "ballistic rise/drift; deceleration; sprite band 67..74; terrain clamp", "STRONG", "model 13 initial", "EventsFunctions.cpp:23572-23612"),
        ("smoke particle 3C", "ballistic rise/drift; sprite band 9..16; terrain clamp", "STRONG", "model 14 initial", "EventsFunctions.cpp:23613-23653"),
        ("wandering crater emitter", "random movement; environmental gate; spawns model-11 children", "PROVISIONAL", "model 15 initial", "EventsFunctions.cpp:23694-23728"),
        ("physical debris and fire object", "gravity; velocity bounds; bounce/friction; splash on water; ground fire on dry impact", "STRONG", "model 16 initial", "EventsFunctions.cpp:23729-23833"),
        ("Meteor impact ring", "expanding divided damage; sound; radial particles; scale change", "STRONG", "model 17 initial", "EventsFunctions.cpp:23834-23905"),
        ("long-lived ground emitter", "periodically spawns effect, debris, and projectile children; global-pair gate", "PROVISIONAL", "model 18 initial", "EventsFunctions.cpp:23906-24006"),
        ("expanding damage and smoke effect", "radial smoke; repeated pending-damage payload; terrain following", "PROVISIONAL", "model 19 initial", "EventsFunctions.cpp:24095-24154"),
        ("missing effect action 10:20", "enabled stored address absent from reconstructed dispatcher", "EXACT_STATIC", "nominal model 20 also missing", "EventsFunctions.cpp:1622"),
        ("missing effect action 10:21", "enabled stored address absent from reconstructed dispatcher", "EXACT_STATIC", "nominal model 21 also missing", "EventsFunctions.cpp:1623"),
        ("Whirlwind root", "updates linked 12-part chain; perturbs nearby entities; looping sound; chain removal", "STRONG", "model 22 initial", "EventsFunctions.cpp:24155-24176"),
        ("Lightning impact", "once-only pending damage; combat accounting; sound; short lifetime", "STRONG", "model 23 initial", "EventsFunctions.cpp:24787-24816"),
        ("missing effect action 10:24", "enabled stored address absent from reconstructed dispatcher", "EXACT_STATIC", "nominal model 24 also missing", "EventsFunctions.cpp:1626"),
        ("Steal Mana impact", "once-only effect-kind 3 pending effect using payload byte", "STRONG", "model 25 initial", "EventsFunctions.cpp:24817-24841"),
        ("Duel impact", "generic motion plus effect-kind 4 pending effect using payload byte", "STRONG", "model 26 initial", "EventsFunctions.cpp:24842-24862"),
        ("rectangular terrain raise command 27", "raises tiles by 48; marks perimeter and terrain type 8", "PROVISIONAL", "model 27 initial", "EventsFunctions.cpp:24897-24928"),
        ("rectangular terrain raise command 28", "parity-adjusted origin; raises tiles and marks perimeter", "PROVISIONAL", "no constructor initializes action 28", "EventsFunctions.cpp:24863-24896"),
        ("rectangular terrain raise command 29", "independent width/height footprint; raises tiles and marks perimeter", "PROVISIONAL", "no constructor initializes action 29", "EventsFunctions.cpp:24929-24988"),
        ("immediate-despawn effect 10:30", "immediate despawn", "EXACT_STATIC", "model 28 initial", "EventsFunctions.cpp:24989-24995"),
        ("immediate-despawn effect 10:31", "immediate despawn", "EXACT_STATIC", "model 29 initial", "EventsFunctions.cpp:24996-25002"),
        ("path-point terrain command", "walks yaw/pitch grid vector; writes material; rebuilds topology", "STRONG", "model 30 initial", "EventsFunctions.cpp:25027-25045"),
        ("immediate-despawn effect 10:33", "immediate despawn", "EXACT_STATIC", "model 31 initial", "EventsFunctions.cpp:25046-25051"),
        ("moving crater emitter", "environment and lifetime gate; spawns model-11 child copies; advances horizontally", "PROVISIONAL", "model 32 initial", "EventsFunctions.cpp:25052-25074"),
        ("generic visible timed effect", "increments age; sets initialized flag; expires", "PROVISIONAL", "model 33 initial", "EventsFunctions.cpp:25854-25875"),
        ("teleporter look-trigger", "facing/proximity scan; moves matching player; changes player state; sounds", "STRONG", "model 34 initial", "EventsFunctions.cpp:25761-25820"),
        ("parent-orbiting child effect", "orbits parent; rotates; terrain clamps; follows parent lifetime", "PROVISIONAL", "no constructor initializes action 37", "EventsFunctions.cpp:25821-25853"),
        ("creature or mana burst", "divides payload and spawns radial class-5 model-9 actors", "PROVISIONAL", "model 36 initial", "EventsFunctions.cpp:25952-26014"),
        ("disabled physical hole 10:39", "disabled row with selector zero and no address", "EXACT_STATIC", "no initializer", "EventsFunctions.cpp:1641", "STRUCTURAL_STATUS"),
        ("dynamic projectile emitter", "rises to altitude then emits two selected class/model projectiles", "PROVISIONAL", "model 38 initial", "EventsFunctions.cpp:25876-25951"),
        ("primary mana-sphere simulation", "attraction; ownership; drag/gravity; terrain/cave/water collision; merge and collection", "STRONG", "model 39 and alias model 58", "EventsFunctions.cpp:26015-26317"),
        ("linked mana or death cleanup", "terrain clamp; follower transfer; link clear; despawn", "STRONG", "model 40 initial", "EventsFunctions.cpp:26835-26866"),
        ("sprite-footprint terrain adjustment", "progressive ground/ceiling and angle changes from MSPR dimensions", "STRONG", "model 41 initial", "EventsFunctions.cpp:27466-27647"),
        ("BUILD footprint growth", "staged terrain/building growth; material, shading, ownership, and parent signaling", "STRONG", "model 42 initial", "EventsFunctions.cpp:27648-27916"),
        ("castle association effect", "terrain check; links caster to nearby castle state; despawn", "STRONG", "model 43 initial", "EventsFunctions.cpp:28219-28254"),
        ("disabled inert action 10:46", "row disabled despite stored address; dispatcher body empty", "EXACT_STATIC", "model 44 initializes this anomalous action", "Events.cpp:2686-2688"),
        ("disabled physical hole 10:47", "disabled row with selector zero and no address", "EXACT_STATIC", "no initializer", "EventsFunctions.cpp:1649", "STRUCTURAL_STATUS"),
        ("progressive terrain modification", "changes ground/ceiling/topology/object positions; may transition to action 52", "STRONG", "terrain transition/current stage", "EventsFunctions.cpp:27181-27436"),
        ("disabled physical hole 10:49", "disabled row with selector zero and no address", "EXACT_STATIC", "no initializer", "EventsFunctions.cpp:1651", "STRUCTURAL_STATUS"),
        ("disabled physical hole 10:50", "disabled row with selector zero and no address", "EXACT_STATIC", "no initializer", "EventsFunctions.cpp:1652", "STRUCTURAL_STATUS"),
        ("building terrain modification entry", "same progressive function as action 48; BUILD-driven entry and action-52 transition", "STRONG", "model 45 initial", "EventsFunctions.cpp:27181-27436"),
        ("live house or building update", "damage/ownership; stage transition; population and mana spawning; sound/color", "STRONG", "transition from actions 48/51", "EventsFunctions.cpp:27959-28070"),
        ("building or castle demolition", "remove BUILD footprint; drop mana/creatures; optional replacement; ownership/topology update", "STRONG", "transition from action 52", "EventsFunctions.cpp:28071-28218"),
        ("immediate-despawn effect 10:54", "immediate despawn", "EXACT_STATIC", "model 50 initial", "EventsFunctions.cpp:25732-25738"),
        ("moving damage and terrain trail", "material/lifetime gate; collision effect; pending damage; sound; forward movement", "PROVISIONAL", "model 51 initial", "EventsFunctions.cpp:25739-25760"),
        ("explicit inert effect 10:56", "enabled/current action whose dispatcher deliberately performs no call", "EXACT_STATIC", "model 52 initial", "Events.cpp:2693-2695"),
        ("explicit inert effect 10:57", "enabled row whose dispatcher deliberately performs no call", "EXACT_STATIC", "no constructor initializes action 57", "Events.cpp:2696-2699"),
        ("expanding explosion ring", "sound; divided pending damage; scale change; radial particles", "PROVISIONAL", "model 53 initial", "EventsFunctions.cpp:28278-28348"),
        ("temporary mana-attraction field", "writes attraction vector/source link for mana spheres in radius", "STRONG", "model 54 and alias model 69", "EventsFunctions.cpp:28349-28385"),
        ("immediate-despawn effect 10:60", "immediate despawn", "EXACT_STATIC", "model 55 initial", "EventsFunctions.cpp:28393-28399"),
        ("explicit inert effect 10:61", "enabled/current action whose dispatcher deliberately performs no call", "EXACT_STATIC", "model 56 initial", "Events.cpp:2716-2718"),
        ("alternate mana-sphere simulation", "attraction; free motion; collision; water splash; merging and combat side effects", "STRONG", "model 57 initial", "EventsFunctions.cpp:26318-26614"),
        ("selector-mismatched physical row 10:63", "stores selector 41; action index 63 fails exact selector validation and is despawned", "EXACT_STATIC", "no constructor initializes action 63", "EventsFunctions.cpp:26015-26317; EventsFunctions.cpp:40138-40141", "STRUCTURAL_STATUS"),
        ("smoke emitter 3B", "shared particle updater spawning model-13 smoke", "STRONG", "model 59 initial", "EventsFunctions.cpp:23654-23659"),
        ("smoke emitter 3C", "shared particle updater spawning model-14 smoke", "STRONG", "model 60 initial", "EventsFunctions.cpp:23660-23665"),
        ("explicit inert effect 10:66", "enabled/current action whose dispatcher deliberately performs no call", "EXACT_STATIC", "model 61 initial", "Events.cpp:2489-2491"),
        ("explicit inert effect 10:67", "enabled/current action whose dispatcher deliberately performs no call", "EXACT_STATIC", "model 62 initial", "Events.cpp:2492-2494"),
        ("one-shot controller action 68", "calls shared controller; sets returned target life to 2; despawns", "PROVISIONAL", "model 63 initial", "EventsFunctions.cpp:25003-25014"),
        ("one-shot controller action 69", "calls shared controller; sets returned target life to 1; despawns", "PROVISIONAL", "model 64 initial", "EventsFunctions.cpp:25015-25026"),
        ("Speed spell player impact", "raises movement-speed stage; applies boost and palette flash; sound; despawn", "STRONG", "model 65 initial", "EventsFunctions.cpp:28400-28423"),
        ("Mobilize or control impact", "applies player effect; sets mobilize counters and boost; sound; despawn", "PROVISIONAL", "model 66 initial", "EventsFunctions.cpp:28424-28451"),
        ("Gravity Well main state", "captures and affects entities; damages; reshapes terrain; advances to actions 73/74", "STRONG", "model 67 initial", "EventsFunctions.cpp:28452-28763"),
        ("Gravity Well continuation", "short continuation; advances to action 74 after lifetime", "STRONG", "transition from action 72", "EventsFunctions.cpp:28764-28782"),
        ("Gravity Well terrain phase", "clears materials; reshapes/restores terrain and shading; releases actors", "STRONG", "transition from actions 72/73", "EventsFunctions.cpp:28783-29010"),
        ("immediate-despawn effect 10:75", "immediate despawn", "EXACT_STATIC", "model 68 initial", "EventsFunctions.cpp:28386-28392"),
        ("duplicate mana-attraction action", "physical duplicate of action 59", "EXACT_STATIC", "no constructor initializes action 76", "EventsFunctions.cpp:28349-28385"),
        ("Possession impact variant 1", "generic effect update plus possession result variant one", "STRONG", "model 70 initial", "EventsFunctions.cpp:23559-23571"),
        ("Tremor impact", "oscillates terrain height; periodic damage and sound; scale animation", "STRONG", "model 71 initial", "EventsFunctions.cpp:29443-29589"),
        ("Summon Army delayed spawn", "constructs selected class-5 model and forces its special action/stage", "STRONG", "model 72 initial", "EventsFunctions.cpp:29590-29622"),
        ("timed effect 10:80", "lifetime countdown and despawn", "PROVISIONAL", "model 73 initial", "EventsFunctions.cpp:29623-29636"),
        ("Alliance area impact", "moves nearby matching actors to special state; assigns owner and sound", "STRONG", "model 74 initial", "EventsFunctions.cpp:29637-29700"),
        ("inert Whirlwind chain child", "disabled table row; root action 22 manually updates eleven linked children", "STRONG", "model 75 child of model 22", "EventsFunctions.cpp:24155-24176"),
        ("enhanced Fireball sphere root", "updates rotating 5x5 linked cluster; parent/terrain attachment; expansion/contraction", "STRONG", "model 76 initial", "EventsFunctions.cpp:24562-24622"),
        ("inert enhanced-Fireball sphere child", "disabled table row; root action 83 manually updates 25 linked children", "STRONG", "model 77 child of model 76", "EventsFunctions.cpp:24562-24622"),
        ("Magic Mine state machine", "movement; arming; targeting; terrain/cave/water physics; impact and damage", "STRONG", "model 78 initial", "EventsFunctions.cpp:29749-30105"),
        ("castle-defense controller", "target acquisition; launch/tracking; collision; damage; reset and impact", "STRONG", "model 79 initial", "EventsFunctions.cpp:30106-30479"),
        ("immediate-despawn cave command 10:87", "immediate despawn", "EXACT_STATIC", "model 80 initial", "EventsFunctions.cpp:25075-25082"),
        ("cave tunnel carver", "samples a line and carves an interpolated tubular corridor; repairs topology", "STRONG", "model 81 initial", "EventsFunctions.cpp:25083-25264"),
        ("rectangular cave-space command", "lowers ground and raises ceiling in footprint; repairs topology", "STRONG", "model 82 initial", "EventsFunctions.cpp:25265-25338"),
        ("radial cave interpolation command", "progressively interpolates ground/ceiling extrema and cave-open flags", "STRONG", "model 83 initial", "EventsFunctions.cpp:25419-25543"),
        ("cave ceiling cavity command", "progressively lowers second heightmap in radial area", "STRONG", "model 84 initial", "EventsFunctions.cpp:25544-25731"),
        ("cave floor mound command", "same function as action 91; progressively raises ground heightmap", "STRONG", "model 85 initial", "EventsFunctions.cpp:25544-25731"),
        ("ground-anchored timed effect", "requires terrain contact; emits model-87 smoke and sound; expires on mismatch", "PROVISIONAL", "model 86 initial", "EventsFunctions.cpp:22826-22859"),
        ("duplicate smoke-ballistic action", "physical duplicate of action 13 with distinct model/sprite band", "EXACT_STATIC", "model 87 initial", "EventsFunctions.cpp:23572-23612"),
        ("missing effect action 10:95", "enabled stored address absent from reconstructed dispatcher", "EXACT_STATIC", "nominal model 88 also missing", "EventsFunctions.cpp:1697"),
        ("Cave In terrain state machine", "deforms floor and ceiling radial shell; smoke/debris; moves affected actors", "STRONG", "model 89 initial", "EventsFunctions.cpp:22860-23098"),
        ("missing effect action 10:97", "enabled stored address absent from reconstructed dispatcher", "EXACT_STATIC", "nominal model 90 also missing", "EventsFunctions.cpp:1699"),
        ("reward or mana burst", "creates three owned mana spheres; updates spell-family progression/accounting", "STRONG", "model 91 initial", "EventsFunctions.cpp:24007-24094"),
    )
    for slot, values in enumerate(class10):
        add(10, slot, *values)

    class11_first = (
        ("overlap switch activate-one", "overlap model-0 trigger in compare mode 1; activate linked id with value 1; disable", "STRONG", "model 0", "EventsFunctions.cpp:44554-44563"),
        ("checkpoint overlap activate-one", "overlap model-0 trigger in compare mode 0; activate linked id with value 1; disable", "STRONG", "model 1", "EventsFunctions.cpp:44566-44575"),
        ("presence switch mode 1", "player-presence switch with ten-tick cooldown; activate link with value 0", "STRONG", "model 2", "EventsFunctions.cpp:54456-54476"),
        ("presence switch mode 0", "player-presence switch with ten-tick cooldown; activate link with value 0", "STRONG", "model 3", "EventsFunctions.cpp:54354-54374"),
        ("level-end player switch", "wait for level-end player flag; activate link with value 1; remove", "STRONG", "model 4", "EventsFunctions.cpp:54377-54395"),
    )
    for slot, values in enumerate(class11_first):
        add(11, slot, *values)
    for slot in range(5, 12):
        add(
            11,
            slot,
            f"missing trigger action 11:{slot:02d}",
            "enabled stored address absent from reconstructed dispatcher",
            "EXACT_STATIC",
            f"constructor model {slot} also missing",
            f"EventsFunctions.cpp:{1804 + slot - 5}",
            "STRUCTURAL_STATUS",
        )
    add(11, 12, "linked-action overlap switch", "mode-1 overlap sets linked entity action 12; disables self; unhides global actor", "STRONG", "model 12", "EventsFunctions.cpp:54479-54500")
    for slot in range(13, 30):
        add(11, slot, f"trigger-flag gate {slot - 13}", f"wait for trigger flag {slot - 13} to clear; delay 16 ticks; sound 41; activate and remove", "STRONG", f"model {slot}", "EventsFunctions.cpp:54505-54657")
    add(11, 30, "aggregate trigger-flag gate", "wait until scanned trigger flags clear; delay, sound, activate, and remove", "STRONG", "model 30", "EventsFunctions.cpp:54732-54735")
    add(11, 31, "linked-action overlap switch with hide gate", "optional immediate hide; overlap sets linked action 11 and unhides global actor", "STRONG", "model 31", "EventsFunctions.cpp:54738-54753")
    add(11, 32, "objective-stage completion switch", "if requested stage active, set objective type-2 done; activate link; remove", "STRONG", "model 32", "EventsFunctions.cpp:54401-54428")
    for slot in range(33, 45):
        add(11, slot, f"trigger-flag gate {slot - 16}", f"wait for trigger flag {slot - 16} to clear; delay 16 ticks; sound 41; activate and remove", "STRONG", f"model {slot}", "EventsFunctions.cpp:54660-54729")

    class14 = (
        ("explicit inert special-fixture action 14:00", "shared dispatcher address deliberately performs no call", "EXACT_STATIC", "model 0 initial", "Events.cpp:2857-2859"),
        ("explicit inert special-fixture action 14:01", "shared dispatcher address deliberately performs no call", "EXACT_STATIC", "physical state only", "Events.cpp:2857-2859"),
        ("explicit inert special-fixture action 14:02", "shared dispatcher address deliberately performs no call", "EXACT_STATIC", "physical state only", "Events.cpp:2857-2859"),
        ("explicit inert special-fixture action 14:03", "shared dispatcher address deliberately performs no call", "EXACT_STATIC", "physical state only", "Events.cpp:2857-2859"),
        ("explicit inert special-fixture action 14:04", "shared dispatcher address deliberately performs no call", "EXACT_STATIC", "physical state only", "Events.cpp:2857-2859"),
        ("explicit inert special-fixture action 14:05", "shared dispatcher address deliberately performs no call", "EXACT_STATIC", "physical state only", "Events.cpp:2857-2859"),
        ("persistent terrain controller", "staged terrain material, height, angle, cave-ceiling, shading, and sound mutation", "STRONG", "model 1", "EventsFunctions.cpp:41263-42503"),
        ("cave corridor controller", "adjusts ground and ceiling toward target shapes; repairs cave angle flags; sound", "STRONG", "model 2", "EventsFunctions.cpp:42538-42790"),
        ("special marker using sprite 338", "terrain snap; constructor records entity in global marker slot", "STRONG", "model 3", "EventsFunctions.cpp:41146-41153"),
        ("special marker using sprite 339", "terrain snap; constructor records entity in global marker slot", "STRONG", "model 4", "EventsFunctions.cpp:41156-41163"),
        ("collectible spell scroll", "player overlap; experience award; sound; spell UI/network update; removal", "STRONG", "model 5", "EventsFunctions.cpp:41166-41204"),
    )
    for slot, values in enumerate(class14):
        add(14, slot, *values)

    for slot in range(10):
        add(
            13,
            slot,
            f"reserved addressless class-13 action row {slot}",
            "table row contains no executable action address",
            "EXACT_STATIC",
            "reserved class-13 metadata family; purpose unresolved",
            f"EventsFunctions.cpp:{1901 + slot}",
            "STRUCTURAL_STATUS",
        )

    # Class 5 is a 29-model family of eight regular states, followed by three
    # exceptional multipart slots.  The state ordinal is a useful structural
    # convention, but several models replace the generic behavior completely.
    creature_generic = (
        "idle/wander update; damage and death routing; same-model partner acquisition; terrain-height adjustment",
        "active roam/awareness update; obstacle movement; target or partner acquisition; damage and death routing",
        "target pursuit/attack update; crowd avoidance and range gate; invalid target returns to active state",
        "linked-part or hit/reaction update; mirrors partner state where applicable",
        "pre-kill transition; propagates death state and records attacker credit",
        "final kill/removal; normally converts to mana and disables drawing",
        "hit/retaliation pursuit; damage, target tracking, movement, and return to active state",
        "authored StageVar2/script dispatch",
    )
    creature_specific: dict[int, tuple[str, ...]] = {
        0: (
            creature_generic[0],
            "Dragon roam plus attachment orbit handling and terrain-relative vertical bobbing",
            "Dragon target attack plus attack sound, attachment orbit handling, and vertical bobbing",
            "Dragon linked-part/reaction update plus orbit handling and vertical bobbing",
            creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            "scripted Dragon update; selected stages also apply orbit handling and vertical bobbing",
        ),
        1: (
            "Goat idle update with randomized vocalization",
            "Goat roam update with randomized vocalization and slow-state setup",
            "redirects immediately to Goat hit/retaliation state 14",
            "Goat linked/reaction update with randomized vocalization",
            creature_generic[4], creature_generic[5],
            "Goat hit/retaliation pursuit; selects fast/slow speed and vocalizes",
            "scripted Goat update with vocalization and state-dependent speed selection",
        ),
        2: (
            "Bee idle update; entering attack initializes attack timer",
            "Bee roam/awareness update; entering attack initializes timer and sound",
            "Bee target attack with vertical alignment, collision callback, recoil speed, and cooldown",
            "Bee linked/reaction update; entering attack initializes timer",
            creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            "scripted Bee update with periodic yaw variation and attack-timer setup",
        ),
        3: (
            creature_generic[0], creature_generic[1],
            "multipart creature target attack plus attack sound",
            creature_generic[3], creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            creature_generic[7],
        ),
        4: (
            "Archer idle update; entering attack configures stationary target-facing visual",
            "Archer roam/damage AI; selects players, buildings, or partners; hit/death routing",
            "Archer target attack with projectile callback and target-facing visual",
            "Archer linked/reaction update and attack-visual setup",
            "Archer death staging; immediate disable for nonzero internal flag, otherwise pre-kill",
            creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            "scripted Archer update; clears counter and configures attack visual",
        ),
        9: (
            "countdown/materialization effects, then initializes active state and visuals",
            "movement/acquisition/conversion AI; can replace a nearby entity with another model-9 actor",
            "target pursuit and proximity attack; invalid or distant target returns active",
            "linked/reaction update with target-specific visual selection",
            creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            "scripted update plus target-specific attack visual",
        ),
        10: (
            "large stationary multi-phase controller; counter-driven animation, sound, spawning, damage, and terminal disable",
            "immediate transition to controller state 80",
            "immediate transition to controller state 80",
            "immediate transition to controller state 80",
            creature_generic[4], creature_generic[5],
            "immediate transition to controller state 80",
            "immediate transition to controller state 80",
        ),
        12: (
            "attempts building placement around referenced target; on success creates class-10/model-45 and crosses to action 105",
            "builder wander/damage update with countdown, turning, hit, and death routing",
            "builder target pursuit; timer/proximity/hit/death transitions",
            "selects nearest building-like entity, otherwise returns to wander",
            creature_generic[4],
            "final kill/removal plus attacker timer bookkeeping",
            "builder hit/retaliation update; clears target and resets retry state",
            "resets to builder wander and executes it immediately",
        ),
        13: (
            "enters civilian active state 105 and executes it immediately",
            "civilian movement/damage update; searches buildings and can increment a target-building counter",
            "enters civilian active state 105 and executes it immediately",
            "enters civilian active state 105 and executes it immediately",
            "civilian death staging; immediate disable for nonzero counter, otherwise pre-kill",
            creature_generic[5],
            "civilian hit/retaliation update; clears target and selects fast speed when leaving hit",
            "scripted civilian update with state-dependent speed selection",
        ),
        14: (
            "enters trader active state 113 and executes it immediately",
            "trader movement/damage update with distinct building-distance filter",
            "enters trader active state 113 and executes it immediately",
            "enters trader active state 113 and executes it immediately",
            "trader death staging; immediate disable for nonzero counter, otherwise pre-kill",
            "trader final kill/removal with attacker bookkeeping",
            "trader hit/retaliation update; clears target and selects fast speed when leaving hit",
            "scripted trader update with state-dependent speed selection",
        ),
        15: (
            creature_generic[0],
            "custom ground movement and player scan; locks a target and enters ranged attack",
            "target pursuit; periodically spawns class-9/model-13 aimed projectile; damage/death routing",
            creature_generic[3], creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            creature_generic[7],
        ),
        16: (
            creature_generic[0],
            "Wyvern roam and nearest-target acquisition",
            "Wyvern chase and combat; periodically launches class-9/model-0 Fireball with fixed payload",
            creature_generic[3], creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            creature_generic[7],
        ),
        17: (
            "idle update plus attack-target validation and substate reset",
            "active/awareness update plus attack-target validation and substate reset",
            "flight/dive combat machine with sound, steering, accelerated dive, collision attack, and recovery",
            "hit/reaction update plus target validation and substate reset",
            creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            "scripted update plus attack-substate reset",
        ),
        18: (
            "terrain-snapped idle/awareness machine; player scan, target lock, steering, and state transitions",
            "damage/death handling, target clear, obstacle movement, and timed return to idle",
            "pursuit/attack-turn substates with smooth turning, interaction, spin recovery, and return to roam",
            "initializes or resets custom attack state",
            creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            "scripted update, terrain snap, and attack-state reinitialization",
        ),
        19: (
            "Firebug idle update and attack-substate reset",
            "Firebug active/awareness update and attack-substate reset",
            "Firebug orbit/approach/attack/recovery flight machine with altitude following, separation, sounds, and collision attack",
            "Firebug hit reaction and attack-substate reset",
            creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            "scripted Firebug update and attack-substate reset",
        ),
        20: (
            "Spider idle update with target validation and attack-substate reset",
            "Spider active/awareness update with target validation and attack-substate reset",
            "Spider melee lunge; doubles speed for 32 ticks, attacks on collision, then restores speed",
            "Spider hit reaction with target validation and attack-substate reset",
            creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            "scripted Spider update and attack-substate reset",
        ),
        21: (
            "resets Devil to common active state and restores movement field",
            "Devil damage, terrain/jump movement, avoidance, hostile scan, and target acquisition",
            "Devil target chase, peer avoidance, periodic melee attack, and terrain/jump movement",
            "resets Devil to common active state",
            creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            "scripted Devil update with selected jump/terrain animation and action normalization",
        ),
        22: (
            "Mana Worm multipart flight, altitude, spacing, animation, hit/ownership, and movement core",
            "searches for and captures mana-sphere targets; otherwise transitions toward owner",
            "returns toward owner castle and selects deposit state when capacity permits",
            "transfers captured mana into owner castle, then hides linked pieces",
            "attached segment follow and hit/ownership propagation",
            "converts the multipart worm into mana spheres and disables it",
            "enabled stored address is absent from the reconstructed dispatcher",
            creature_generic[7],
        ),
        23: (
            "custom roaming/resource-seek state with avoidance, separation, altitude, timers, and target selection",
            "slow target approach; validates interaction and enters resource-consume state",
            "hostile pursuit with sound, avoidance, steering, and distance-gated attack",
            "captures, raises, and absorbs target resource mana, then searches again",
            creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            creature_generic[7],
        ),
        24: (
            "Caveman idle update with target scan, animation, and speed normalization",
            "Caveman active update with target acquisition, animation, and speed normalization",
            "Caveman targeted melee attack with sound",
            "forces active state and runs generic idle behavior",
            creature_generic[4], creature_generic[5],
            "Caveman hit/recovery, target reacquisition, animation, and speed normalization",
            "scripted Caveman update with animation and speed normalization",
        ),
        25: (
            "Cymmerian master machine: damage, timeout, castle navigation, roaming, terrain mode, and temporary invulnerable spin",
            "active/awareness update and negative-life clamp",
            "sound and melee/collision attack with negative-life clamp",
            "forces active state and clamps negative life",
            "custom death/split into three new model-25 actors or a mana sphere",
            "kill-stat bookkeeping and disable/final-kill behavior",
            "enabled stored address is absent from the reconstructed dispatcher",
            "flag-dependent scripted update",
        ),
        26: (
            "Zombie idle update with animation and speed normalization",
            "Zombie active/awareness update with animation and speed normalization",
            "Zombie combat; retargeting, movement, player-mana drain, and close-range spell interaction",
            "Zombie hit reaction with animation and speed normalization",
            creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            "scripted Zombie update with animation and speed normalization",
        ),
        27: (
            "Hydra timed multipart emergence, terrain placement, sound, visibility, and attack transition",
            "Hydra roam/acquire with multipart damage, heading, pose, and movement",
            "Hydra multipart target combat, steering, attack/travel pose, and target validation",
            "forces Hydra roam state and executes it immediately",
            "sets life negative and enters Hydra pre-kill",
            "transforms root to mana sphere, creates optional effect, and removes all parts",
            "enabled stored address is absent from the reconstructed dispatcher",
            "scripted Hydra multipart pose/state propagation; interacts with exceptional action 233 branches",
        ),
        28: (
            "Deep One idle update and combat-substate initialization",
            "Deep One active/awareness update and combat-substate initialization",
            "Deep One predictive combat, steering, charge/collision, terrain avoidance, recovery, and escape",
            "returns immediately to active state",
            creature_generic[4], creature_generic[5],
            "enabled stored address is absent from the reconstructed dispatcher",
            "scripted Deep One update and combat-state initialization",
        ),
    }
    creature_ranges = {
        0: "11189-11383", 1: "11386-11462", 2: "11465-11578",
        3: "11581-11622", 4: "11625-11966", 9: "11969-12651",
        10: "12654-13903", 12: "13942-14471", 13: "14499-14718",
        14: "14721-14949", 15: "14952-15330", 16: "15333-15557",
        17: "15560-15838", 18: "15841-16258", 19: "16261-16610",
        20: "16613-16751", 21: "16754-16967", 22: "17247-18049",
        23: "18052-18633", 24: "18636-18721", 25: "18828-19216",
        26: "19219-19409", 27: "19443-19735", 28: "20990-21271",
    }
    creature_models = {int(row["runtime_model"]): row for row in creature_model_rows()}
    disabled_creature_models = {5, 6, 7, 8, 11}
    missing_creature_actions = {
        6, 22, 30, 38, 78, 126, 134, 142, 150, 158, 166,
        174, 182, 190, 206, 214, 222, 230,
    }
    for slot in range(232):
        model, state = divmod(slot, 8)
        model_row = creature_models[model]
        if model in disabled_creature_models:
            behavior = "disabled row retaining an original stored address; no action is invoked"
            confidence = "EXACT_STATIC"
            level = "STRUCTURAL_STATUS"
        elif slot in missing_creature_actions:
            behavior = "enabled original address is absent from the reconstructed dispatcher; behavior remains unknown"
            confidence = "EXACT_STATIC"
            level = "STRUCTURAL_STATUS"
        else:
            behavior = creature_specific.get(model, creature_generic)[state]
            confidence = "STRONG"
            level = "INDIVIDUAL_BEHAVIOR"
        source_range = creature_ranges.get(model, f"{1243 + slot}")
        add(
            5,
            slot,
            f"{model_row['semantic_label']} state {state}",
            behavior,
            confidence,
            f"class-5 model {model} regular eight-state band; initial action {model_row['initial_action']}",
            f"EventsFunctions.cpp:{1243 + slot}; EventsFunctions.cpp:{source_range}",
            level,
            f"model identity confidence={model_row['confidence']}; behavior confidence is independent",
        )
    add(
        5, 232, "shared multipart-segment follow state",
        "follows stored parent; disables if parent leaves class 5; computes yaw/pitch and configured trailing position; consumes pending damage",
        "STRONG", "created as sixteen linked body segments by model-0 and model-3 constructors",
        "EventsFunctions.cpp:1475; EventsFunctions.cpp:8696-8734; EventsFunctions.cpp:33701-33717; EventsFunctions.cpp:33844-33869",
        notes="shared infrastructure outside the 29 regular model bands",
    )
    add(
        5, 233, "Hydra exceptional branch state",
        "disabled/null action row, although the Hydra constructor assigns it to five branch entities and action 223 later reads or changes them",
        "EXACT_STATIC", "Hydra constructor and scripted multipart controller",
        "EventsFunctions.cpp:1476; EventsFunctions.cpp:34636-34651; EventsFunctions.cpp:19713-19725",
        "STRUCTURAL_STATUS", "material reconstructed-source/runtime inconsistency requiring oracle confirmation",
    )
    add(
        5, 234, "Hydra exceptional segment state",
        "disabled/null action row, although the Hydra constructor assigns it to nine segments per branch",
        "EXACT_STATIC", "Hydra constructor",
        "EventsFunctions.cpp:1477; EventsFunctions.cpp:34652-34669",
        "STRUCTURAL_STATUS", "material reconstructed-source/runtime inconsistency requiring oracle confirmation",
    )

    # Class 15 uses three entity roles per family, not three spell tiers.
    # The selected tier lives in byte_0x46_70 and is loaded through SetSpell.
    active_spell_behaviors = (
        "first tick launches Fireball projectile with owner, effect, mana, aim, and forward target; cast cost, countdown, queued-tier, and cooldown lifecycle",
        "launches Possession or enhanced magnet/lock projectile variants; enhanced modes may emit helper shots on later ticks",
        "writes castle tier parameters and spawns castle placement or upgrade delivery; countdown update is asymmetric",
        "sustained Speed Up modifier; first effective tick awards experience; restores normal speed and visibility state on end",
        "creates selected creature morph and swaps player/creature visibility; removes creature and restores player on end",
        "heals a percentage of maximum life per effective tick while mana and missing life permit; per-tick cost and first-use experience",
        "maintains shield/invulnerability status flags selected by tier and clears both shield flags on end",
        "launches standard, enhanced, or paired enhanced Lightning projectiles with copied aim and ownership",
        "maintains Rebound/Amplify status flags selected by tier and clears them on end",
        "launches Meteor projectile with copied owner, tier effect, mana, and aim",
        "teleports to own castle, saved position, or other-player castles by tier; awards experience and zeros movement speed",
        "Invisible cast lifecycle; sets/tests a spell-entity flag but clears parent flag on end, an asymmetry requiring oracle confirmation",
        "Beyond Sight cast lifecycle and experience; handler contains no direct sight/radar mutation, so the enabled tier is consumed elsewhere",
        "launches Steal Mana projectile with owner, tier, mana, and aim",
        "launches Duel projectile; selected mode can extend until player state signals completion",
        "launches Tremor delivery projected to terrain",
        "launches Crater delivery projected to terrain",
        "launches Earthquake delivery; cost helper is initial-tick-only",
        "launches Volcano delivery; cost helper is initial-tick-only",
        "launches Summon Army delivery with owner, tier, and aim",
        "launches Gravity Well delivery with owner, tier, and aim",
        "launches Whirlwind delivery; nonzero tier-life divides effect; cost helper is initial-tick-only",
        "creates up to six Fool's Mana spheres around player with speed derived from player movement and deterministic random state",
        "launches Magic Mine delivery with inherited player speed, owner, tier, mana, and terrain-projected target",
        "launches Alliance delivery with owner, parent, tier effect, and aimed target",
        "cave-gated Cave In delivery; outside cave realms this handler does not advance its cast countdown",
    )
    active_spell_ranges = (
        "55880-55946", "55963-56082", "56134-56220", "56237-56325",
        "56342-56463", "56480-56527", "56544-56593", "56609-56752",
        "56769-56816", "56832-56891", "56908-57099", "57116-57164",
        "57180-57209", "57225-57289", "57306-57379", "57396-57452",
        "57469-57525", "57542-57597", "57614-57669", "57686-57748",
        "57765-57820", "57837-57900", "57916-57991", "58008-58070",
        "58087-58154", "58171-58233",
    )
    for family_id, family_name in enumerate(FAMILY_NAMES):
        base = family_id * 3
        add(
            15, base, f"{family_name} equipped/active spell state",
            active_spell_behaviors[family_id], "STRONG",
            f"class-15 family {family_id}; selected data tier is byte_0x46_70, not the action ordinal",
            f"EventsFunctions.cpp:{1950 + base}; EventsFunctions.cpp:{active_spell_ranges[family_id]}; Level.cpp:1504-1535",
            notes="physical action role and data-driven spell tier are separate dimensions",
        )
        add(
            15, base + 1, f"{family_name} ordinary world-pickup state",
            "settles and ages in world; scans players; on eligible collision attaches to player, updates inventory/hand selection, loads current tier, and enters active state",
            "STRONG", f"ordinary pickup transitions from {base + 1} to active state {base}",
            f"EventsFunctions.cpp:{1951 + base}; EventsFunctions.cpp:55723-55772",
            notes="not a spell tier",
        )
        add(
            15, base + 2, f"{family_name} persistent respawning-pickup state",
            "uses ordinary acquisition logic; on successful pickup creates a replacement of the same family at the same position in persistent-pickup state",
            "STRONG", f"persistent pickup transitions from {base + 2} to active state {base} and respawns {base + 2}",
            f"EventsFunctions.cpp:{1952 + base}; EventsFunctions.cpp:55723-55772; Events.cpp:5242-5257",
            notes="not a spell tier",
        )
    add(
        15, 78, "shared detached-spell return state",
        "moves detached spell through shared travel helper; on completion enters its family ordinary-pickup state and marks world/detached status",
        "STRONG", "sub_69300 removes inventory ownership and transitions the spell to action 78 before this per-tick return",
        "EventsFunctions.cpp:2028; EventsFunctions.cpp:55821-55876",
        notes="shared across all 26 families; inventory removal occurs before this tick handler",
    )

    # A semantic label does not make an inert, disabled, or missing native row
    # executable behavior. Keep those exact structural facts out of the
    # reconstructed-behavior numerator.
    structural_only_overrides = {
        *((2, slot) for slot in (4, 5, 7, 8, *range(10, 18))),
        *((3, slot) for slot in (7, 8, 10)),
        *((10, slot) for slot in (4, 20, 21, 24, 46, 56, 57, 61, 66, 67, 82, 84, 95, 97)),
        *((14, slot) for slot in range(6)),
    }
    for row in rows:
        key = (int(row["runtime_class"]), int(row["action_physical_slot"]))
        if key in structural_only_overrides:
            row["annotation_level"] = "STRUCTURAL_STATUS"

    expected = {
        2: 22, 3: 13, 5: 235, 9: 32, 10: 99, 11: 45,
        13: 10, 14: 11, 15: 79,
    }
    actual: dict[int, int] = {}
    for row in rows:
        runtime_class = int(row["runtime_class"])
        actual[runtime_class] = actual.get(runtime_class, 0) + 1
    if actual != expected:
        raise ValueError(f"runtime action annotation denominator drift: {actual}")
    rows.sort(
        key=lambda row: (
            int(row["runtime_class"]),
            int(row["action_physical_slot"]),
        )
    )
    return rows


def claim_rows(records: list[TierRecord]) -> list[dict[str, object]]:
    rows: list[dict[str, object]] = []
    for family_id, family_name in enumerate(FAMILY_NAMES):
        rows.append(
            {
                "claim_id": f"spell-family-{family_id:02d}",
                "namespace": "spell",
                "raw_key": f"family={family_id}",
                "proposed_label": family_name,
                "role": "spell family identity",
                "initialization_source": "global_types.h:135-162",
                "load_override_chain": "identity is stable; parameters are separately loaded",
                "writers": "none for family identity",
                "authoritative_consumers": "class-15 spell holder and casting dispatch",
                "presentation_consumers": (
                    f"text={160 + family_id}; icons={97 + family_id},"
                    f"{123 + family_id}"
                ),
                "linked_claims": f"spell-tier-{family_id:02d}-0..2",
                "evidence_anchors": (
                    "global_types.h:135-162; GameBitmapIndexes.h:28-51; "
                    "EventsFunctions.cpp:54172-54351"
                ),
                "confidence": "STRONG",
                "conflicts": "",
                "extraction_ready": True,
            }
        )
    for record in records:
        rows.append(
            {
                "claim_id": f"spell-tier-{record.family_id:02d}-{record.tier}",
                "namespace": "spell",
                "raw_key": f"family={record.family_id};tier={record.tier}",
                "proposed_label": record.tier_name,
                "role": "loaded spell tier and presentation binding",
                "initialization_source": "owned DATA/SPELLS.DAT",
                "load_override_chain": record.load_override_chain,
                "writers": "SetDefaultSpells_5C0A0; LevelInit map-mode patch",
                "authoritative_consumers": (
                    "SetSpell_6D5E0; GetSpellManaCost_6D710; casting dispatch"
                ),
                "presentation_consumers": (
                    f"text={record.tier_text_id}; icon={record.tier_icon_index}"
                ),
                "linked_claims": f"spell-family-{record.family_id:02d}",
                "evidence_anchors": (
                    "Spells.h:6-25; EventsFunctions.cpp:42911-42919; "
                    "Level.cpp:1505-1535; LevelInit.cpp:9-40"
                ),
                "confidence": record.confidence,
                "conflicts": "",
                "extraction_ready": record.extraction_ready,
            }
        )
    rows.extend(
        [
            {
                "claim_id": "runtime-spell-holder",
                "namespace": "runtime_entity",
                "raw_key": "class=15;model=spell_family;action=family*3",
                "proposed_label": "player spell holder/inventory entity",
                "role": "stores selected tier, cost, charge and cast state",
                "initialization_source": "AddSpellXX_XX_51120",
                "load_override_chain": "constructed from the live spell table",
                "writers": "SetSpell_6D5E0 and player spell progression",
                "authoritative_consumers": "input/cast path and HUD",
                "presentation_consumers": "DrawSpellIcon_2E260",
                "linked_claims": "spell-family-00..25",
                "evidence_anchors": (
                    "EventsFunctions.cpp:54172-54351; Level.cpp:1305-1319; "
                    "Level.cpp:1505-1535"
                ),
                "confidence": "STRONG",
                "conflicts": "",
                "extraction_ready": True,
            },
            {
                "claim_id": "spell-holder-action-role-bands",
                "namespace": "spell",
                "raw_key": "class=15;actions=3*family+[0,1,2];detached=78",
                "proposed_label": "class-15 active, ordinary-pickup, persistent-pickup, and detached roles",
                "role": "physical spell-holder entity state architecture",
                "initialization_source": "class-15 dispatch table and spell-holder constructor",
                "load_override_chain": "family selects three-state band; byte_0x46_70 independently selects loaded data tier",
                "writers": "pickup, detach, SetSpell, and cast lifecycle paths",
                "authoritative_consumers": "all 79 resolved class-15 action handlers",
                "presentation_consumers": "family icon from model; tier label from byte_0x46_70",
                "linked_claims": "runtime-spell-holder;spell-family-00..25",
                "evidence_anchors": (
                    "EventsFunctions.cpp:1949-2029; EventsFunctions.cpp:55723-55876; "
                    "EventsFunctions.cpp:55880-58233; Level.cpp:1504-1535; GameUI.cpp:374-394"
                ),
                "confidence": "STRONG",
                "conflicts": "",
                "extraction_ready": True,
            },
            {
                "claim_id": "runtime-fireball-projectile",
                "namespace": "runtime_entity",
                "raw_key": "class=9;model=0;action=0",
                "proposed_label": "ordinary Fireball projectile",
                "role": "moving projectile",
                "initialization_source": "SummonFireball_4D2E0",
                "load_override_chain": "family 0 tier record -> sub_6DCA0 -> constructor",
                "writers": "cast path copies owner, power, aim and velocity",
                "authoritative_consumers": "projectile action/collision path",
                "presentation_consumers": "render index 340; FireBall sound 9",
                "linked_claims": "spell-family-00;audio-fireball-09",
                "evidence_anchors": (
                    "EventsFunctions.cpp:34737-34755; "
                    "EventsFunctions.cpp:44074-44149; Events.cpp:4376"
                ),
                "confidence": "STRONG",
                "conflicts": "damage/status formula remains unresolved",
                "extraction_ready": False,
            },
            {
                "claim_id": "runtime-water-splash",
                "namespace": "runtime_entity",
                "raw_key": "class=10;model=5",
                "proposed_label": "Fireball water-impact splash",
                "role": "impact effect",
                "initialization_source": "fireball projectile water collision",
                "load_override_chain": "water query -> effect creation -> projectile destroy",
                "writers": "projectile collision action",
                "authoritative_consumers": "effect action table",
                "presentation_consumers": "Splash2 sound 27",
                "linked_claims": "runtime-fireball-projectile;audio-splash-27",
                "evidence_anchors": "EventsFunctions.cpp:63171-63195",
                "confidence": "STRONG",
                "conflicts": "",
                "extraction_ready": True,
            },
            {
                "claim_id": "audio-fireball-09",
                "namespace": "audio",
                "raw_key": "in_game_sound=9",
                "proposed_label": "FireBall",
                "role": "spell launch cue",
                "initialization_source": "SoundInGameIndexes.h",
                "load_override_chain": "event cue -> sound system",
                "writers": "casting factory",
                "authoritative_consumers": "PrepareEventSound_6E450",
                "presentation_consumers": "spatial audio",
                "linked_claims": "runtime-fireball-projectile",
                "evidence_anchors": (
                    "SoundInGameIndexes.h:5-20; EventsFunctions.cpp:44278-44289"
                ),
                "confidence": "STRONG",
                "conflicts": "",
                "extraction_ready": True,
            },
            {
                "claim_id": "audio-splash-27",
                "namespace": "audio",
                "raw_key": "in_game_sound=27",
                "proposed_label": "Splash2",
                "role": "water impact cue",
                "initialization_source": "SoundInGameIndexes.h",
                "load_override_chain": "water collision -> sound system",
                "writers": "projectile collision action",
                "authoritative_consumers": "PrepareEventSound_6E450",
                "presentation_consumers": "spatial audio",
                "linked_claims": "runtime-water-splash",
                "evidence_anchors": (
                    "SoundInGameIndexes.h:20-36; EventsFunctions.cpp:63171-63195"
                ),
                "confidence": "STRONG",
                "conflicts": "",
                "extraction_ready": True,
            },
        ]
    )
    rows.extend(core_behavior_claim_rows())
    return rows


def core_claim(
    claim_id: str,
    namespace: str,
    raw_key: str,
    label: str,
    role: str,
    anchors: str,
    *,
    confidence: str = "STRONG",
    conflicts: str = "",
    linked: str = "",
    presentation: str = "",
) -> dict[str, object]:
    return {
        "claim_id": claim_id,
        "namespace": namespace,
        "raw_key": raw_key,
        "proposed_label": label,
        "role": role,
        "initialization_source": "runtime state or fixed engine contract",
        "load_override_chain": "input/state -> ordered tick consumer",
        "writers": "see evidence anchors",
        "authoritative_consumers": "see evidence anchors",
        "presentation_consumers": presentation,
        "linked_claims": linked,
        "evidence_anchors": anchors,
        "confidence": confidence,
        "conflicts": conflicts,
        "extraction_ready": confidence in {"STRONG", "CONFIRMED", "EXACT_STATIC"} and not conflicts,
    }


def core_behavior_claim_rows() -> list[dict[str, object]]:
    rows = [
        core_claim(
            "flight-fixed-trig-displacement",
            "flight",
            "MoveEntity_57FA0(yaw,pitch,speed)",
            "fixed-point 3D displacement",
            "11-bit yaw/pitch and 16.16 sine-table movement",
            "Player.cpp:5-19",
            linked="flight-forward;flight-strafe",
        ),
        core_claim(
            "flight-roll-input",
            "flight",
            "player.rollDelta_0x4_4",
            "roll input delta",
            "ordered roll accumulator input",
            "EventsFunctions.cpp:59670-59680",
        ),
        core_claim(
            "flight-pitch-input",
            "flight",
            "player.pitchDelta_0x6_6",
            "pitch input delta",
            "ordered pitch accumulator input",
            "EventsFunctions.cpp:59670-59680",
        ),
        core_claim(
            "flight-target-speed",
            "flight",
            "player.speed_0xc_12",
            "target forward speed",
            "input-selected speed approached by the actual entity speed",
            "EventsFunctions.cpp:59684-59692; EventsFunctions.cpp:60795-60912",
        ),
        core_claim(
            "flight-actual-speed",
            "flight",
            "entity.actSpeed_0x82_130",
            "actual forward speed",
            "quantized forward-speed state",
            "EventsFunctions.cpp:59684-59692; EventsFunctions.cpp:59715-59728",
        ),
        core_claim(
            "flight-speed-step",
            "flight",
            "speedIcrement_D4B84=16",
            "forward acceleration step",
            "per-tick approach step from actual speed to target speed",
            "EventsFunctions.cpp:59684-59692",
        ),
        core_claim(
            "flight-strafe",
            "flight",
            "player.strafeSpeed_0x10_16",
            "lateral strafe speed",
            "second displacement at yaw plus 512",
            "EventsFunctions.cpp:59729-59742; EventsFunctions.cpp:60795-60912",
            linked="flight-fixed-trig-displacement",
        ),
        core_claim(
            "flight-yaw-from-roll",
            "flight",
            "yaw += signed_roll / 8",
            "bank-to-turn coupling",
            "roll accumulator drives yaw before translation",
            "EventsFunctions.cpp:59682-59683",
        ),
        core_claim(
            "flight-terrain-relative-pitch",
            "flight",
            "altDiff clamped -256..256",
            "terrain-relative pitch shaping",
            "changes effective movement pitch near the surface",
            "EventsFunctions.cpp:59693-59714",
            linked="terrain-ground-height",
        ),
        core_claim(
            "flight-forward",
            "flight",
            "MoveEntity(yaw,effective_pitch,actual_speed)",
            "forward displacement",
            "primary carpet translation",
            "EventsFunctions.cpp:59715-59728",
            linked="flight-fixed-trig-displacement",
        ),
        core_claim(
            "flight-recoil",
            "flight",
            "player.moveBoost_0x1E_30",
            "hit or recoil impulse",
            "bounded directional impulse with signed decay to zero",
            "EventsFunctions.cpp:59743-59759",
        ),
        core_claim(
            "flight-transient-impulse",
            "flight",
            "player.xAdd/yAdd/zAdd",
            "one-tick Cartesian impulse",
            "consumed after addition to predicted position",
            "EventsFunctions.cpp:59761-59766",
        ),
        core_claim(
            "flight-speed-modifier",
            "flight",
            "player.moveSpeed_0x14C_332",
            "speed-spell movement modifier",
            "scales forward/strafe movement and decays by counters",
            "EventsFunctions.cpp:59715-59720; EventsFunctions.cpp:59770-59785",
        ),
        core_claim(
            "flight-immobilize",
            "flight",
            "player.mobilizeCounter_0x14E_334",
            "movement immobilization state",
            "suppresses translation and changes vertical response",
            "EventsFunctions.cpp:59720-59723; EventsFunctions.cpp:59787-59798",
        ),
        core_claim(
            "flight-ground-clamp",
            "flight",
            "ground + entity clearance",
            "ground collision clamp",
            "prevents the carpet entity from crossing the terrain surface",
            "EventsFunctions.cpp:59793-59818",
            linked="terrain-ground-height",
        ),
        core_claim(
            "flight-cave-clamp",
            "flight",
            "ceiling - 384",
            "cave ceiling clamp",
            "prevents the carpet entity from crossing the cave ceiling",
            "EventsFunctions.cpp:59807-59812",
            linked="terrain-cave-ceiling",
        ),
        core_claim(
            "creature-speed-target-ceiling",
            "entity_movement",
            "entity.minSpeed_0x84_132",
            "common actor cruise target and upper speed bound",
            (
                "despite the recovered minSpeed name, common actor movement "
                "approaches this value by two units per tick and damage response "
                "clamps actual speed down to it as the upper bound"
            ),
            "EventsFunctions.cpp:62971-62980; EventsFunctions.cpp:17551-17561",
            confidence="STRONG",
            conflicts=(
                "the field is overloaded by some constructors/actions; preserve "
                "the raw name and qualify the semantic label by consumer"
            ),
            linked="creature-speed-floor",
        ),
        core_claim(
            "creature-speed-floor",
            "entity_movement",
            "entity.maxSpeed_0x86_134",
            "common actor lower speed bound",
            (
                "despite the recovered maxSpeed name, the shared damage-response "
                "path clamps actual speed up to this value as its lower bound"
            ),
            "EventsFunctions.cpp:17551-17561",
            confidence="STRONG",
            conflicts=(
                "the field is overloaded by some constructors/actions; preserve "
                "the raw name and qualify the semantic label by consumer"
            ),
            linked="creature-speed-target-ceiling",
        ),
        core_claim(
            "terrain-type-grid",
            "terrain",
            "mapTerrainType_10B4E0[256*256]",
            "terrain material/type grid",
            "authoritative per-cell surface classification",
            "Terrain.cpp:6; Terrain.cpp:2058-2107",
        ),
        core_claim(
            "terrain-height-grid",
            "terrain",
            "mapHeight_11B4E0[256*256]",
            "terrain height grid",
            "heightfield backing interpolated ground queries",
            "Terrain.cpp:7; Terrain.cpp:115-2149",
        ),
        core_claim(
            "terrain-shading-grid",
            "terrain",
            "mapShading_FEB4E0[256*256]",
            "terrain shading grid",
            "per-cell lighting/presentation state",
            "Terrain.cpp:8",
        ),
        core_claim(
            "terrain-angle-water-grid",
            "terrain",
            "mapAngle_13B4E0[256*256]",
            "terrain topology, rotation, and mutation flags",
            (
                "low topology bits select ground/ceiling closure and higher bits "
                "carry rotation or dirty/recalculation state"
            ),
            "Terrain.cpp:9; Terrain.cpp:1158-1239; Terrain.cpp:1874-1885",
        ),
        core_claim(
            "terrain-cell-entity-grid",
            "terrain",
            "mapEntityIndex_15B4E0[256*256]",
            "per-cell entity index",
            "spatial lookup into the runtime entity pool",
            "Terrain.cpp:10",
        ),
        core_claim(
            "terrain-ground-height",
            "terrain",
            "getTerrainAlt_10C40(position)",
            "interpolated ground height query",
            "ground contract consumed by flight, projectiles, and entities",
            "Terrain.cpp:115-2149",
        ),
        core_claim(
            "terrain-water",
            "terrain",
            "sub_104D0_terrain_tile_is_water(position)",
            "general terrain material classifier",
            (
                "maps the terrain type at a position to a material/contact bit; "
                "returned class 1 is the strongly identified water case"
            ),
            "Terrain.cpp:2058-2107; EventsFunctions.cpp:59858-59866",
            linked="audio-ocean-loop;audio-crickets-loop",
            conflicts="the recovered function name is narrower than its behavior",
        ),
        core_claim(
            "terrain-cave-ceiling",
            "terrain",
            "sub_10C60(position)",
            "interpolated cave ceiling query",
            "secondary height surface used in cave collision",
            "Terrain.cpp:2158-2164; EventsFunctions.cpp:59807-59812",
        ),
        core_claim(
            "terrain-cave-height-grid",
            "terrain",
            "x_BYTE_14B4E0_second_heightmap[256*256]",
            "cave ceiling or second height surface",
            "backs cave ceiling construction, topology repair, and interpolation",
            "BasicTerrain.h:110-111; Terrain.cpp:1158-1180; Terrain.cpp:2158-2164",
            linked="terrain-cave-ceiling",
        ),
        core_claim(
            "terrain-topology-render-lookup",
            "terrain",
            "building_F2CD0x[2800][2]",
            "four-corner topology to rendered tile/rotation lookup",
            (
                "maps local corner-height/material topology into a terrain tile "
                "selector and rotation used during generation and repair"
            ),
            "Terrain.cpp:12-13; Terrain.cpp:1010-1154; Terrain.cpp:1975-1996",
        ),
        core_claim(
            "level-record-layout",
            "level",
            "Type_Level_2FECE size=0x6604",
            "uncompressed authored level record",
            (
                "contains realm/map type, procedural generator parameters, 1200 "
                "twenty-byte authored records, eight wizard settings, stages, "
                "and stage variables"
            ),
            "BasicTerrain.h:7-18; BasicTerrain.h:69-107",
        ),
        core_claim(
            "terrain-generation-pipeline",
            "terrain",
            "GenerateLevelMap_43830(Type_Level_2FECE)",
            "deterministic base-map generation pipeline",
            (
                "seeds ground generation, normalizes and smooths heights, cuts "
                "rivers, assigns topology/materials, builds cave ceiling when "
                "required, and derives shading"
            ),
            "Terrain.cpp:19-56; Terrain.cpp:59-110; Terrain.cpp:1010-1241",
            linked="level-record-layout;terrain-height-grid;terrain-cave-height-grid",
        ),
        core_claim(
            "level-world-initialization-order",
            "level",
            "LevelInitGame_56A30",
            "base-map then ordered feature prepasses then entity instantiation",
            (
                "decompresses the level, selects its realm, generates the base "
                "terrain, applies dependency-ordered authored terrain/building "
                "commands, then instantiates remaining runtime entities"
            ),
            "Events.cpp:151-282; EventsFunctions.cpp:32958-33025; EventsFunctions.cpp:39359-39449",
            linked="terrain-generation-pipeline;level-record-layout",
        ),
        core_claim(
            "level-map-realm",
            "level",
            "MapType_t {Day=0,Night=1,Cave=2}",
            "authored day, night, or cave realm selector",
            (
                "selects distinct block, sky, terrain-map, palette, table, and "
                "sprite families; it is per-level content, not a continuous "
                "astronomical day/night cycle"
            ),
            "MapType.h:4; ReadAndDecompress.cpp:27-169; Level.cpp:443-461",
            conflicts="no continuous day/night transition subsystem is proven",
        ),
        core_claim(
            "runtime-standing-stone",
            "runtime_entity",
            "class=2;model=1",
            "Standing stone",
            "authored scenery constructor",
            "EventsFunctions.cpp:33474-33488; editor.cpp:2468-2478",
            confidence="STRONG",
        ),
        core_claim(
            "runtime-dolmen",
            "runtime_entity",
            "class=2;model=2",
            "Dolmen or stonehenge",
            "authored scenery constructor",
            "EventsFunctions.cpp:33492-33507; editor.cpp:2480-2484",
            confidence="STRONG",
        ),
        core_claim(
            "runtime-bad-stone",
            "runtime_entity",
            "class=2;model=3",
            "Bad stone",
            "authored scenery constructor",
            "EventsFunctions.cpp:33511-33525; editor.cpp:2474-2478",
            confidence="STRONG",
        ),
        core_claim(
            "runtime-cave-scenery-6",
            "runtime_entity",
            "class=2;model=6",
            "cave mushroom scenery",
            (
                "constructor refuses non-cave maps and selects particle records "
                "324 through 327; all four records consume cave TMAP 424 at "
                "different scales, whose palette-correct image is a mushroom"
            ),
            "EventsFunctions.cpp:33562-33591; Type_WORD_D951C.cpp:328-331",
            confidence="CONFIRMED",
        ),
        core_claim(
            "terrain-lava-material-family",
            "terrain",
            "mapTerrainType values 10,11,12",
            "three-member lava terrain-material family",
            (
                "the renderer directly indexes terrain artwork from mapTerrainType; "
                "palette-correct tiles 10 through 12 are a dark/cracked lava family, "
                "authored type 0x0F writes runtime type 11, and terrain logic preserves "
                "10/11/12 as one family while assigning distinct classifier flags"
            ),
            (
                "GameRenderOriginal.cpp:887; GameRenderOriginal.cpp:1013; "
                "GameRenderOriginal.cpp:1081-1088; Terrain.cpp:1831-1839; "
                "Terrain.cpp:2096-2103"
            ),
            confidence="STRONG",
            conflicts=(
                "individual hot, crust, or animation-state names require a "
                "controlled lava-contact trace"
            ),
        ),
        core_claim(
            "world-cave-linked-tunnel-control",
            "world_effect",
            "level=0A:50",
            "linked cave tunnel/path control points",
            (
                "walks a linked control-point chain and creates tube-carver "
                "segments between successive points"
            ),
            "Events.cpp:5261-5361; Events.cpp:5621-5687",
            confidence="STRONG",
        ),
        core_claim(
            "world-cave-rectangular-excavator",
            "world_effect",
            "level=0A:52",
            "rectangular cave chamber or shaft excavator",
            "binds authored dimensions and excavates a rectangular cave volume",
            "Events.cpp:374-379; EventsFunctions.cpp:36382-36402",
            confidence="STRONG",
        ),
        core_claim(
            "world-cave-radial-volume-shaper",
            "world_effect",
            "level=0A:53",
            "radial or domed cave-volume shaper",
            "binds an authored radius and reshapes a radial cave volume",
            "Events.cpp:381-384; EventsFunctions.cpp:36405-36426",
            confidence="STRONG",
            conflicts="exact editor-facing noun remains unresolved",
        ),
        core_claim(
            "world-cave-ceiling-cavity-shaper",
            "world_effect",
            "level=0A:54",
            "radial cave-ceiling depression or cavity shaper",
            "applies an authored radial change to the cave ceiling",
            "Events.cpp:386-388; EventsFunctions.cpp:36429-36440",
            confidence="STRONG",
            conflicts="exact editor-facing noun remains unresolved",
        ),
        core_claim(
            "world-cave-floor-mound-shaper",
            "world_effect",
            "level=0A:55",
            "radial cave-floor mound or rise shaper",
            "applies an authored radial change to the cave floor",
            "Events.cpp:389-391; EventsFunctions.cpp:36441-36472",
            confidence="STRONG",
            conflicts="exact editor-facing noun remains unresolved",
        ),
        core_claim(
            "world-cave-oriented-passage-opener",
            "world_effect",
            "level=0E:02",
            "oriented configurable-length cave passage opener and smoother",
            (
                "scans in the authored orientation for solid boundaries, then "
                "adjusts floor and cave ceiling over multiple smoothing phases"
            ),
            "Events.cpp:302-320; EventsFunctions.cpp:37405-37425; EventsFunctions.cpp:42537-42723",
            confidence="STRONG",
            conflicts="exact editor-facing noun remains unresolved",
        ),
        core_claim(
            "runtime-barrel",
            "runtime_entity",
            "class=2;model=7",
            "Barrel",
            "non-cave ambient scenery",
            "EventsFunctions.cpp:33594-33602; editor.cpp:2492-2496",
            confidence="STRONG",
        ),
        core_claim(
            "runtime-vase",
            "runtime_entity",
            "class=2;model=8",
            "Vase",
            "non-cave ambient scenery",
            "EventsFunctions.cpp:33605-33613; editor.cpp:2498-2502",
            confidence="STRONG",
        ),
        core_claim(
            "world-building-command",
            "world_effect",
            "class=10;model=45;level=0A:2D",
            "BUILD-driven building and terrain-footprint command",
            (
                "uses BUILD records for sprite dimensions and placement, transfers "
                "parameters to the runtime entity, averages the footprint ground, "
                "and repairs terrain topology"
            ),
            "Events.cpp:256-281; EventsFunctions.cpp:31080-31206; EventsFunctions.cpp:32454-32823; EventsFunctions.cpp:33092-33101",
            confidence="STRONG",
        ),
        core_claim(
            "world-volcano",
            "world_effect",
            "class=10;model=9;level=0A:09",
            "volcano terrain effect",
            (
                "raises a radial mountain, repairs a cave ceiling when present, "
                "emits quake audio, cuts a summit/crater, and spawns eruption effects"
            ),
            "EventsFunctions.cpp:23193-23429; EventsFunctions.cpp:35521-35537",
            confidence="STRONG",
        ),
        core_claim(
            "world-mini-volcano-noop",
            "world_effect",
            "class=10;model=8;level=0A:08",
            "legacy mini-volcano slot currently disabled",
            "the runtime dispatch reaches a constructor that returns null",
            "EventsFunctions.cpp:35515-35518",
            confidence="STRONG",
            conflicts="historical intended behavior is not recovered",
        ),
        core_claim(
            "world-wind-vortex",
            "world_effect",
            "class=10;model=22;level=0A:16",
            "wind vortex or tornado entity",
            (
                "constructor creates a root plus eleven vertically stacked linked "
                "segments; owned/editor and Tornado audio evidence corroborate identity"
            ),
            "EventsFunctions.cpp:35860-35938; editor.cpp:2570-2574",
            confidence="STRONG",
            conflicts="force application still needs a controlled trace",
        ),
        core_claim(
            "world-lightning-effect",
            "world_effect",
            "class=10;model=23;level=0A:17",
            "lightning damage effect",
            "short-lived eight-tick runtime damaging effect",
            "EventsFunctions.cpp:24787-24813; EventsFunctions.cpp:36094-36114",
            confidence="STRONG",
        ),
        core_claim(
            "world-splash-effect",
            "world_effect",
            "class=10;model=5;level=0A:05",
            "water splash",
            "spawned on identified water impacts and paired with splash audio",
            "EventsFunctions.cpp:23772-23787",
            confidence="STRONG",
        ),
        core_claim(
            "world-fire-effect",
            "world_effect",
            "class=10;model=6;level=0A:06",
            "fire effect",
            "authored/runtime fire effect corroborated by editor imagery",
            "editor.cpp:2564-2568",
            confidence="PROVISIONAL",
        ),
        core_claim(
            "world-path-command",
            "world_effect",
            "class=10;model=29;level=0A:1D",
            "path terrain command",
            "writes terrain topology/material state along a path and recalculates tiles",
            "EventsFunctions.cpp:25027-25043",
            confidence="STRONG",
        ),
        core_claim(
            "world-canyon-tunnel-command",
            "world_effect",
            "class=10;model=31;level=0A:1F",
            "canyon or tunnel terrain command",
            "modifies both ground and cave ceiling surfaces along a route",
            "EventsFunctions.cpp:25083-25260",
            confidence="STRONG",
        ),
        core_claim(
            "audio-ocean-loop",
            "audio",
            "in_game_sound=1",
            "Ocean ambient loop",
            "selected while the player is over water",
            "SoundInGameIndexes.h:5; EventsFunctions.cpp:59858-59862",
            presentation="spatial/ambient sound",
        ),
        core_claim(
            "audio-crickets-loop",
            "audio",
            "in_game_sound=2",
            "Crickets ambient loop",
            "selected over non-water outdoor terrain",
            "SoundInGameIndexes.h:6; EventsFunctions.cpp:59863-59866",
            presentation="spatial/ambient sound",
        ),
        core_claim(
            "audio-cave-drips",
            "audio",
            "in_game_sound=65..69",
            "cave drip ambience family",
            "random low-frequency cave ambience",
            "SoundInGameIndexes.h:69-73; EventsFunctions.cpp:59848-59856",
            presentation="spatial/ambient sound",
        ),
        core_claim(
            "runtime-tree",
            "runtime_entity",
            "class=2;model=0",
            "Tree",
            "scenery entity",
            "Events.cpp:4233; EventsFunctions.cpp:33441-33470",
            confidence="STRONG",
        ),
        core_claim(
            "runtime-archer",
            "runtime_entity",
            "class=5;model=4",
            "Archer",
            "living ranged actor",
            "Events.cpp:1333-1346; Events.cpp:4291; EventsFunctions.cpp:11885-11940",
            confidence="STRONG",
        ),
        core_claim(
            "runtime-villager",
            "runtime_entity",
            "class=5;model=13;action=105",
            "Villager or townie",
            "civilian economy actor",
            "EventsFunctions.cpp:34044-34099",
            confidence="STRONG",
        ),
        core_claim(
            "runtime-trader",
            "runtime_entity",
            "class=5;model=14;action=113",
            "Trader",
            "civilian/economy actor",
            "Events.cpp:4311; EventsFunctions.cpp:34101-34134",
            confidence="STRONG",
        ),
        core_claim(
            "fireball-projectile-impact-transition",
            "spell",
            "class=9;model=0;action=0 -> impact class=10;model=0;action=0",
            "ordinary Fireball and Rapid Fire projectile-to-impact transition",
            (
                "on entity, terrain, cave-ceiling, or lifetime contact, the projectile "
                "creates the configured impact entity and copies its owner and payload"
            ),
            (
                "EventsFunctions.cpp:63170-63248; "
                "EventsFunctions.cpp:44075-44135"
            ),
            confidence="STRONG",
            linked="runtime-fireball-projectile;fireball-area-damage-writer",
        ),
        core_claim(
            "fireball-tier-0-payload",
            "spell",
            "family=0;tier=0;effect_selector_raw=250",
            "FireBall direct impact damage payload",
            (
                "the loaded tier value is copied through the projectile into the "
                "impact entity and submitted once to the shared damage accumulator"
            ),
            (
                "Spells.h:7-17; EventsFunctions.cpp:44075-44135; "
                "EventsFunctions.cpp:22692-22755"
            ),
            confidence="STRONG",
            linked="fireball-projectile-impact-transition;fireball-area-damage-writer",
        ),
        core_claim(
            "fireball-tier-1-payload",
            "spell",
            "family=0;tier=1;effect_selector_raw=160",
            "Rapid Fire direct impact damage payload",
            (
                "the loaded tier value follows the same ordinary projectile and "
                "single-application impact path as tier zero"
            ),
            (
                "Spells.h:7-17; EventsFunctions.cpp:44075-44135; "
                "EventsFunctions.cpp:22692-22755"
            ),
            confidence="STRONG",
            linked="fireball-projectile-impact-transition;fireball-area-damage-writer",
        ),
        core_claim(
            "fireball-impact-radius",
            "spell",
            "class=10;model=0;SetEntityShiftRot(128,128)",
            "ordinary Fireball impact collision radius",
            (
                "the impact constructor assigns a 128-unit shift/footprint used by "
                "the shared overlap and area-effect scan"
            ),
            "EventsFunctions.cpp:35340-35364; EventsFunctions.cpp:3953-4161",
            confidence="STRONG",
            linked="fireball-area-damage-writer",
        ),
        core_claim(
            "fireball-area-damage-writer",
            "spell",
            "sub_10C80(effect,channel=0,payload)",
            "shared deferred area-damage accumulator",
            (
                "scans spatial entity chains, filters owner/collision masks and "
                "overlap, then adds the payload to each target's pending-damage "
                "field and records the source entity"
            ),
            "EventsFunctions.cpp:3953-4161",
            confidence="STRONG",
            conflicts=(
                "per-target shields, immunities, death conversion, and later "
                "consumption remain class/action-specific"
            ),
            linked="fireball-tier-0-payload;fireball-tier-1-payload",
        ),
        core_claim(
            "fireball-impact-once",
            "spell",
            "class=10;model=0;effect flag bit 1",
            "ordinary Fireball damage is submitted once per impact entity",
            (
                "the impact action tests its initialized flag, calls the shared "
                "damage writer only on first activation, then sets the flag"
            ),
            "EventsFunctions.cpp:22692-22755",
            confidence="STRONG",
            linked="fireball-area-damage-writer",
        ),
        core_claim(
            "fireball-hit-statistics",
            "spell",
            "sub_65780(projectile,hit,current_target)",
            "projectile hit and accuracy bookkeeping",
            (
                "increments owner-player shot/hit counters for selected projectile "
                "models; it is not the damage equation"
            ),
            "EventsFunctions.cpp:62884-62928",
            confidence="STRONG",
            conflicts="previous unresolved ledger grouped this with damage handling",
        ),
        core_claim(
            "projectile-last-hit-link",
            "spell",
            "sub_686D0(projectile,hit)",
            "owner player last-hit entity link",
            (
                "records the collided runtime entity index for player-class owners; "
                "it is not the damage equation"
            ),
            "EventsFunctions.cpp:55241-55267",
            confidence="STRONG",
        ),
        core_claim(
            "projectile-special-interception",
            "spell",
            "sub_68AC0(projectile,hit)",
            "special projectile interception or retrieval path",
            (
                "recognizes a class-10 model-78 owner-linked interceptor, emits "
                "Clang sound 26, transfers metadata, and suppresses the ordinary impact"
            ),
            "EventsFunctions.cpp:55445-55518",
            confidence="STRONG",
            conflicts="exact player-facing identity of class-10 model 78 still provisional",
        ),
        core_claim(
            "dynamic-light-registration",
            "rendering",
            "AddEvent2_847D0(entity,intensity,variation,flags)",
            "night/cave dynamic-light registration",
            (
                "allocates one of fifty light records, links it to a runtime entity, "
                "and initializes its 5x5 prior-contribution buffer"
            ),
            "EventsFunctions.cpp:47218-47249",
            confidence="STRONG",
            linked="dynamic-light-remove-previous;dynamic-light-apply-current",
        ),
        core_claim(
            "dynamic-light-remove-previous",
            "rendering",
            "sub_848A0",
            "remove previous dynamic-light contribution",
            (
                "on night/cave realms, subtracts each active light's saved 5x5 "
                "contribution from the terrain shading grid before simulation moves it"
            ),
            "EventsFunctions.cpp:47268-47436",
            confidence="STRONG",
            linked="dynamic-light-registration;dynamic-light-apply-current",
        ),
        core_claim(
            "dynamic-light-apply-current",
            "rendering",
            "sub_84B80",
            "update and apply current dynamic lights",
            (
                "retires invalid light owners, follows live entity positions, "
                "uses deterministic variation, calculates a 5x5 contribution, "
                "and applies it to terrain shading"
            ),
            "EventsFunctions.cpp:47440-47727",
            confidence="STRONG",
            linked="dynamic-light-registration;dynamic-light-remove-previous",
        ),
        core_claim(
            "level-end-promotion",
            "objective",
            "sub_57570",
            "promote objective completion to level-end state",
            (
                "when not suppressed, copies the current player's completed-level "
                "state into the session transition flag"
            ),
            "EventsFunctions.cpp:39889-39900",
            confidence="STRONG",
        ),
        core_claim(
            "checkpoint-conditional-reload",
            "progression",
            "sub_575C0",
            "conditional single-player checkpoint reload",
            (
                "when the paired defeat/reload and saved-checkpoint flags are set, "
                "loads the current level checkpoint and clears transition bits"
            ),
            "EventsFunctions.cpp:39902-39923",
            confidence="PROVISIONAL",
            conflicts="friendly names for the packed transition bits remain provisional",
        ),
        core_claim(
            "ai-ground-obstacle-avoidance",
            "entity_ai",
            "sub_1B8C0(entity)",
            "ground actor obstacle-avoidance probe",
            (
                "predicts movement, tests terrain, then tries yaw offsets +341, "
                "-341, and +1024 before reporting the path blocked"
            ),
            "EventsFunctions.cpp:8741-8944",
            confidence="STRONG",
            linked="flight-fixed-trig-displacement;terrain-ground-height",
        ),
        core_claim(
            "ai-stage-script-dispatch",
            "entity_ai",
            "sub_1D5D0(entity);StageVar2=1..17",
            "stage-script behavior dispatch",
            "selects one of seventeen behavior branches from the actor stage variable",
            "EventsFunctions.cpp:9977-10010",
            confidence="STRONG",
        ),
        core_claim(
            "ai-target-link-upkeep",
            "entity_ai",
            "sub_1ED30(entity)",
            "actor target and parent-link upkeep",
            "validates and changes runtime target/parent relationships",
            "EventsFunctions.cpp:11060-11170",
            confidence="STRONG",
        ),
        core_claim(
            "ai-height-follow",
            "entity_ai",
            "sub_1EEE0(entity)",
            "actor terrain-height following",
            "adjusts an actor's vertical state against authoritative terrain height",
            "EventsFunctions.cpp:11172-11191",
            confidence="STRONG",
            linked="terrain-ground-height",
        ),
        core_claim(
            "ai-target-steering",
            "entity_ai",
            "sub_65610(actor,target)",
            "target yaw/pitch steering",
            (
                "computes desired yaw and pitch toward a target then slews current "
                "orientation using per-actor turn parameters"
            ),
            "EventsFunctions.cpp:62829-62882",
            confidence="STRONG",
        ),
        core_claim(
            "ai-guided-movement",
            "entity_ai",
            "sub_65820(actor)",
            "guided actor/projectile movement",
            (
                "updates target steering, converges actual speed, moves in fixed "
                "point, then handles terrain, cave, water, and entity contacts"
            ),
            "EventsFunctions.cpp:62930-63051",
            confidence="STRONG",
            linked="ai-target-steering;creature-speed-target-ceiling;creature-speed-floor",
        ),
        core_claim(
            "ai-target-selection",
            "entity_ai",
            "sub_68490/sub_685D0",
            "field-of-view and distance target selection",
            "scores candidates inside view cones out to 5120 world units",
            "EventsFunctions.cpp:55149-55239",
            confidence="STRONG",
        ),
        core_claim(
            "projectile-rebound",
            "spell",
            "sub_68740(projectile,defender,...)",
            "rebound-shield projectile reflection",
            (
                "checks defender rebound state, spends defender mana, reverses "
                "the projectile, and transfers ownership"
            ),
            "EventsFunctions.cpp:55269-55420",
            confidence="STRONG",
        ),
        core_claim(
            "creature-pending-damage-consumer",
            "damage",
            "representative class-5 action consumes +0x5E pending damage",
            "creature deferred-damage consumption",
            (
                "subtracts accumulated pending damage from life, clears the pending "
                "marker, and enters model-specific hit or pre-kill handling"
            ),
            "EventsFunctions.cpp:9572-9638",
            confidence="STRONG",
            linked="fireball-area-damage-writer;creature-prekill-kill",
        ),
        core_claim(
            "creature-prekill-kill",
            "damage",
            "PreKillEntity_1C890;KillEntity_1C930",
            "multipart-aware creature death pipeline",
            (
                "propagates multipart death and kill credit, then periodically "
                "drops mana, creates the death effect, and hides the entity"
            ),
            "EventsFunctions.cpp:9533-9569",
            confidence="STRONG",
            linked="creature-pending-damage-consumer;entity-pool-lifecycle;creature-mana-drop",
        ),
        core_claim(
            "creature-mana-drop",
            "economy",
            "TransformEntityToManaSphere_36BA0",
            "creature death-to-mana conversion",
            "converts configured creature value into one or more mana-sphere entities",
            "EventsFunctions.cpp:26867-26929",
            confidence="STRONG",
            conflicts="manaFraction local shadowing requires original/native trace",
            linked="runtime-mana-sphere",
        ),
        core_claim(
            "runtime-mana-sphere",
            "runtime_entity",
            "class=10 mana-sphere constructor",
            "mana sphere runtime entity",
            "constructs the collectible/transferrable mana object used by death drops",
            "EventsFunctions.cpp:36615-36645",
            confidence="STRONG",
            linked="creature-mana-drop",
        ),
        core_claim(
            "authoritative-tick-order",
            "scheduler",
            "DrawAndEventsInGame_47560",
            "ordered frame and simulation-turn pipeline",
            (
                "samples and translates input, applies player actions, removes "
                "prior dynamic lights, advances entities, reapplies lighting, "
                "evaluates objectives, handles transitions/audio, then renders"
            ),
            "EventsFunctions.cpp:31732-31880",
            confidence="STRONG",
        ),
        core_claim(
            "watcom-compatible-rng",
            "determinism",
            "watcomsrand/watcomrand",
            "Watcom-compatible pseudo-random sequence",
            (
                "seeds and advances the compatibility generator used where exact "
                "original ordering matters"
            ),
            "EventsFunctions.cpp:413-431",
            confidence="STRONG",
            linked="authoritative-tick-order",
        ),
        core_claim(
            "deterministic-action-recording",
            "replay",
            "RecordPlayerActions(level,player,turn,size,bytes)",
            "per-level, per-player, per-turn action-byte recording",
            (
                "stores an exact-sized copy of the current action bytes under "
                "stable level, player, and turn keys"
            ),
            "InputRecorder.cpp:139-167; EventsFunctions.cpp:37569-37609",
            confidence="STRONG",
            linked="authoritative-tick-order;deterministic-action-playback",
        ),
        core_claim(
            "deterministic-action-playback",
            "replay",
            "LoadRecordingFile and PlayerEvents substitution",
            "recorded action playback at the player-action boundary",
            (
                "loads MC2-HD-Recording data and replaces current player input "
                "bytes at the matching level/player/turn before actions execute"
            ),
            "InputRecorder.cpp:240-325; EventsFunctions.cpp:37569-37609",
            confidence="STRONG",
            linked="deterministic-action-recording",
        ),
        core_claim(
            "exact-state-regression",
            "regression",
            "SaveCompare/CompareWith(name,indexed bytes)",
            "indexed native/oracle byte comparison",
            (
                "saves repeated byte sequences then seeks the matching occurrence "
                "and raises an error at the first differing byte"
            ),
            "Compare.cpp:20-59",
            confidence="STRONG",
        ),
        core_claim(
            "legacy-multiplayer-action-boundary",
            "network",
            "PlayerEvents_51BB0 multiplayer ReceiveSendAll",
            "legacy multiplayer player-action/state exchange",
            (
                "updates connections, exchanges the player-input array, and "
                "conditionally exchanges player state before the turn actions run"
            ),
            "EventsFunctions.cpp:37536-37568",
            confidence="STRONG",
            conflicts="preservation behavior is not a recommendation for a new public protocol",
            linked="authoritative-tick-order",
        ),
        core_claim(
            "legacy-network-message-chunking",
            "network",
            "NetworkSendMessage_74EF1/NetworkReceiveMessage_74D41",
            "legacy fixed-packet message chunking",
            "splits and rejoins caller-provided buffers around the reconstructed packet limit",
            "Network.cpp:807-874",
            confidence="STRONG",
            conflicts="transport and trust assumptions remain legacy-only",
        ),
        core_claim(
            "level-state-load",
            "persistence",
            "LoadLevel_555D0",
            "saved/packaged level-state load boundary",
            "selects and loads the map and level-state components for the requested slot",
            "Level.cpp:255-407",
            confidence="PROVISIONAL",
            conflicts="component format and overwrite lineage remain under field-level audit",
        ),
        core_claim(
            "level-state-save",
            "persistence",
            "SaveLevel_55080",
            "level-state save boundary",
            "serializes the current map and level-state components to the requested slot",
            "Level.cpp:408-442",
            confidence="PROVISIONAL",
            conflicts="component field authority remains under field-level audit",
        ),
        core_claim(
            "camera-player-snapshot",
            "camera",
            "sub_57B20(player,entity)",
            "player camera-state snapshot",
            (
                "when not paused, copies the controlled entity position and yaw "
                "into the active camera slot, applies player roll/pitch plus "
                "recoil and transient view offsets, and preserves the selected FOV"
            ),
            "EventsFunctions.cpp:40197-40339",
            confidence="STRONG",
            linked="flight-recoil;flight-forward",
        ),
        core_claim(
            "player-spawn-respawn-initialization",
            "player",
            "sub_5C950(player,existing_entity_or_sentinel)",
            "player or AI wizard spawn/respawn initialization",
            (
                "selects the authored player spawn, creates or repositions the "
                "class-3 entity, binds player state, installs default life/mana "
                "and movement state, applies AI aggression/perception/reflex/life "
                "settings, restores resources, and refreshes presentation links"
            ),
            "EventsFunctions.cpp:43685-44074",
            confidence="STRONG",
            linked="level-record-layout;entity-constructor-dispatch",
        ),
        core_claim(
            "session-level-loop",
            "scheduler",
            "sub_46830_main_loop(level)",
            "outer menu, level, play, and transition scheduler",
            (
                "runs frontend selection, loads and initializes the chosen level, "
                "enters the in-game loop, then handles music/audio shutdown, "
                "statistics, save/transition flags, and secret-level changes"
            ),
            "EventsFunctions.cpp:31275-31584",
            confidence="STRONG",
            linked="level-world-initialization-order;authoritative-tick-order",
        ),
    ]
    rows.extend(
        [
            core_claim(
                "entity-runtime-selectors",
                "runtime_entity",
                "entity.class_0x3F_63;model_0x40_64;actionIndex_0x45_69",
                "runtime entity class, model, and current-action selectors",
                (
                    "class chooses the table family, model chooses its constructor "
                    "or archetype, and action chooses the per-tick behavior row"
                ),
                "global_types.h:357-365; EventsFunctions.h:222-240",
                confidence="STRONG",
                linked="entity-constructor-dispatch;entity-action-dispatch",
            ),
            core_claim(
                "entity-constructor-dispatch",
                "runtime_entity",
                "str_D4C48ar[class].constructors[model]",
                "runtime class/model constructor dispatch",
                "validates class and model then invokes the stored constructor",
                "Events.cpp:5186-5190; EventsFunctions.cpp:2060-2077",
                confidence="STRONG",
                linked="entity-runtime-selectors",
            ),
            core_claim(
                "entity-action-dispatch",
                "runtime_entity",
                "str_D4C48ar[class].actions[action]",
                "runtime class/action per-tick dispatch",
                "validates the action row and invokes its stored behavior each update",
                "EventsFunctions.cpp:40124-40187",
                confidence="STRONG",
                linked="entity-runtime-selectors;entity-pool-lifecycle",
            ),
            core_claim(
                "entity-pool-lifecycle",
                "runtime_entity",
                "Entities_EA3E4[0..999]",
                "fixed 1000-record entity pool lifecycle",
                (
                    "allocation clears a 168-byte entity record; hiding marks it "
                    "first, and a later update unlinks and recycles the slot"
                ),
                "Events.cpp:561-623; Events.cpp:5194-5240; EventsFunctions.cpp:39942-40187; EventsFunctions.cpp:40340-40344",
                confidence="STRONG",
                linked="entity-action-dispatch",
            ),
        ]
    )
    rows.extend(
        [
            core_claim(
                "packed-storage-union-views",
                "representation",
                "dw_w_b::{byte,word,dword}",
                "overlapping byte, word, and dword storage views",
                (
                    "these members are aliases over the same four bytes; their "
                    "meaning is determined by the containing field and consumer"
                ),
                "global_types.h:11-16",
                confidence="EXACT_STATIC",
            ),
            core_claim(
                "packed-coordinate-union-view",
                "representation",
                "uaxis_2d::{_axis_2d,word}",
                "packed two-byte coordinate and word views",
                (
                    "the word and two unsigned byte coordinates overlap exactly; "
                    "the union is representation, not one global gameplay parameter"
                ),
                "global_types.h:60-73",
                confidence="EXACT_STATIC",
            ),
            core_claim(
                "renderer-working-state",
                "rendering",
                "str_F2C20ar",
                "software world-renderer working state",
                (
                    "stores viewport buffer, projection, camera trigonometry, fog "
                    "bounds, tile cutoff, stride, and temporary draw dimensions"
                ),
                "Type_F2C20ar.h:11-52; GameRenderOriginal.cpp:586-679",
                confidence="STRONG",
            ),
            core_claim(
                "entity-auxiliary-state-pointer",
                "runtime_entity",
                "entity.dword_0xA4_164x",
                "entity link to class/model-specific auxiliary state",
                (
                    "allocation, serialization, input, spells, castle, camera, and "
                    "AI consumers dereference this pointer according to entity role; "
                    "its pointee semantics are context-dependent"
                ),
                "global_types.h:357-392; Events.cpp:561-600; Basic.cpp:3089-3282",
                confidence="STRONG",
            ),
            core_claim(
                "entity-polymorphic-payload-slots",
                "runtime_entity",
                "selected raw fields inside type_entity_0x6E8E",
                "class, model, and action-dependent entity payload slots",
                (
                    "the same packed offsets are initialized and consumed with "
                    "different meanings across creature, projectile, effect, "
                    "building, player, and trigger actions; they require per-action "
                    "aliases and must not receive one global friendly name"
                ),
                "global_types.h:321-392; EventsFunctions.cpp:39942-40187",
                confidence="STRONG",
            ),
            core_claim(
                "host-frontend-state-aggregate",
                "platform",
                "x_DWORD_17DE38str",
                "host palette, UI, input, save-dialog, and legacy-network aggregate",
                (
                    "collects palette and bitmap pointers, mouse/button state, "
                    "frontend animation, selected save slot, and legacy session "
                    "fields; it is not one authoritative gameplay variable"
                ),
                "Basic.h:103-183",
                confidence="STRONG",
            ),
            core_claim(
                "host-input-device-state",
                "platform",
                "unk_18058Cstr",
                "host mouse, button, directional-key, and joystick snapshot",
                (
                    "the aggregate holds current mouse coordinates and buttons, "
                    "saved/recentered mouse coordinates, directional-key bits, and "
                    "the active joystick/control mode; PlayerInput translates it "
                    "into deterministic per-turn player input bytes"
                ),
                "EventsFunctions.h:309-332; PlayerInput.cpp:1973-2140",
                confidence="STRONG",
                linked="deterministic-action-playback",
            ),
            core_claim(
                "player-spell-state-block",
                "spell",
                "Type_str_164::str_611",
                "per-player spell progression, inventory, selection, and HUD state",
                (
                    "the nested block stores experience, enabled spell-holder links, "
                    "levels, ordering, left/right selections, and transient menu "
                    "selection; its selected indices are copied into deterministic "
                    "player input before casting"
                ),
                "global_types.h:174-231; global_types.h:308; PlayerInput.cpp:818-921; PlayerInput.cpp:2037-2074",
                confidence="STRONG",
                linked="runtime-spell-holder;deterministic-action-playback",
            ),
            core_claim(
                "player-spell-charge-counter",
                "spell",
                "Type_str_164::byte_0x154_340",
                "bounded time-since-spell-release payload",
                (
                    "the per-player byte increments once per simulation update to "
                    "a hard cap of 200; projectile-producing cast paths copy it into "
                    "the new entity payload and reset it to zero, while other cast "
                    "and lifecycle paths also clear it"
                ),
                "EventsFunctions.cpp:5423-5425; EventsFunctions.cpp:55917-55918; EventsFunctions.cpp:56201-56202; EventsFunctions.cpp:60039-60040",
                confidence="STRONG",
                linked="player-spell-state-block;runtime-spell-holder",
            ),
            core_claim(
                "objective-polymorphic-payload",
                "objective",
                "type_str_3654C::str_36552_un",
                "objective-stage payload interpreted by objective kind",
                (
                    "the same four bytes carry a scalar, authored-level entity "
                    "pointer, live runtime-entity pointer, or 16-bit content pointer; "
                    "load/save code translates pointer forms and objective evaluation "
                    "selects the interpretation from the surrounding stage record"
                ),
                "LevelStructs.h:137-151; Level.cpp:1111-1161; EventsFunctions.cpp:40622-40689; EventsFunctions.cpp:40760-40955",
                confidence="STRONG",
                linked="objective-evaluation-order",
            ),
            core_claim(
                "objective-kind-and-runtime-flags",
                "objective",
                "type_str_3654C::stages_3654C_byte0;type_str_3654C::str_3654D_byte1",
                "objective kind selector and live-binding/completion flags",
                (
                    "kinds 0..9 select mana, entity-death, player-defeat, delivery, "
                    "position, indexed-state, subtype/list, last-player, or building "
                    "predicates; flag bit 0 marks a bound live reference and bit 1 "
                    "is a consumed force-complete pulse"
                ),
                "EventsFunctions.cpp:40578-40691; EventsFunctions.cpp:40744-40890; GameUI.cpp:826-858",
                confidence="STRONG",
                conflicts="other flag bits remain unresolved; one recovered nested-index expression at EventsFunctions.cpp:48702 is likely malformed",
                linked="objective-polymorphic-payload;objective-evaluation-order",
            ),
            core_claim(
                "level-stage-variable-records",
                "objective",
                "type_str_0x3647Ac::str_0x3647C_4",
                "scripted stage-variable payload interpreted by stage-variable kind",
                (
                    "the four-byte union is used as packed coordinates, a scalar, "
                    "or a runtime-entity pointer; level activation resolves authored "
                    "indices and coordinates, and scripted entity actions consume "
                    "the representation selected by the record type"
                ),
                "BasicTerrain.h:50-67; LevelStructs.h:272-281; EventsFunctions.cpp:4658-4740; EventsFunctions.cpp:4965-5183; Level.cpp:1179-1198",
                confidence="STRONG",
                linked="objective-evaluation-order;creature-scripted-stage-action",
            ),
            core_claim(
                "host-key-binding-and-capture-state",
                "platform",
                "x_BYTE_EB39E_keys;x_DWORD_17DE38str.x_BYTE_17DF10_get_key_scancode",
                "legacy host key bindings and active key-capture scancode",
                (
                    "the ten-byte table is initialized with DOS scancodes and read "
                    "by local control paths; the frontend aggregate separately holds "
                    "the scancode currently captured while rebinding controls"
                ),
                "EventsFunctions.cpp:43167-43183; EventsFunctions.cpp:46430-46513; EventsFunctions.cpp:49722-50500",
                confidence="STRONG",
                linked="host-input-device-state",
            ),
            core_claim(
                "secondary-render-target-buffer",
                "rendering",
                "x_DWORD_E9C3C",
                "interior pointer to secondary off-screen 8-bit render target",
                (
                    "terrain, stereo/shifted-screen, blur, and duplicate UI paths "
                    "render through this alternate target; its owning allocation is "
                    "separate and the interior offset changes by display mode"
                ),
                "Basic.cpp:1488-1544; Level.cpp:1050-1067; GameRenderOriginal.cpp:146-181; MenusAndIntros.cpp:2067-2144",
                confidence="STRONG",
            ),
            core_claim(
                "multiplayer-lobby-action-records",
                "network",
                "type_x_DWORD_17DE38str::array_BYTE_17DE68x",
                "eight-slot lobby synchronization and action records",
                (
                    "each record carries a dirty/update flag, player-to-wizard "
                    "mappings, lobby action opcode, and selected level; local menus "
                    "write them and network peers exchange and apply them"
                ),
                "Basic.h:36-43; MenusAndIntros.cpp:5266-5319; MenusAndIntros.cpp:5736-5754; Network.cpp:873-887; Network.cpp:1015-1087",
                confidence="STRONG",
            ),
            core_claim(
                "campaign-map-player-marker-animation",
                "rendering",
                "type_x_DWORD_17DE38str::unk_17E078x",
                "campaign-map player marker position and directional animation",
                (
                    "stores marker coordinates, current directional sprite, and "
                    "animation frame bounds; map movement angle selects its frame "
                    "range before animation and drawing"
                ),
                "Basic.h:55-64; MenusAndIntros.cpp:964-968; MenusAndIntros.cpp:2286-2376; MenusAndIntros.cpp:3633-3747",
                confidence="STRONG",
                linked="campaign-map-screen-state",
            ),
            core_claim(
                "scroll-dialog-animation-state",
                "rendering",
                "type_menuButtons_E1F84::str_26;type_str_word_26::word_36_5",
                "embedded scroll-dialog layout and animated open-height state",
                (
                    "the menu-button record embeds dialog position, target height, "
                    "current height, text/sprite/mode, and click/close state; drawing "
                    "grows the current height toward the target then reverses it"
                ),
                "EventsFunctions.h:367-377; MenusAndIntros.cpp:1579-1597; MenusAndIntros.cpp:5220-5240; MenusAndIntros.cpp:5415-5470; EventsFunctions.cpp:46409-46417",
                confidence="STRONG",
            ),
            core_claim(
                "menu-button-context-state",
                "rendering",
                "type_menuButtons_E1F84::byte_25",
                "context-dependent menu-entry terminator, enable flag, or dialog phase",
                (
                    "map-menu arrays use nonzero as entry/enabled discrimination, "
                    "the joystick dialog uses values 0..3 as a phase machine, and "
                    "load-flow code uses it to distinguish map-origin behavior"
                ),
                "MenusAndIntros.cpp:321-326; MenusAndIntros.cpp:1506-1566; MenusAndIntros.cpp:2383-2413; MenusAndIntros.cpp:5044-5260",
                confidence="STRONG",
            ),
            core_claim(
                "billboard-raster-lookup",
                "rendering",
                "type_unk_F0E20x",
                "per-rolled-billboard framebuffer offset and step lookup",
                (
                    "billboard setup computes a destination byte offset for each "
                    "oriented sample, its delta from the prior offset, and a third "
                    "orthogonal accumulator that current renderers do not consume"
                ),
                "Type_Unk_F0E20x.h:8-13; GameRenderHD.cpp:2204-2205; GameRenderHD.cpp:2331-2348; GameRenderHD.cpp:4496-4572",
                confidence="STRONG",
            ),
            core_claim(
                "legacy-gameport-joystick-state",
                "platform",
                "unk_180560x",
                "legacy analog gameport joystick sample and calibration packet",
                (
                    "the 44-byte buffer stores raw axes, min/max/center calibration, "
                    "device mode/subtype, buttons, and normalized axes consumed by "
                    "gameplay and frontend input paths"
                ),
                "EventsFunctions.cpp:49790-49899; EventsFunctions.cpp:50828-50840; EventsFunctions.cpp:50906-51238; MenusAndIntros.cpp:5481-5496",
                confidence="STRONG",
                linked="host-input-device-state",
            ),
            core_claim(
                "campaign-map-ambient-animations",
                "rendering",
                "str_WORD_E20A4",
                "sentinel-terminated campaign-map ambient animation table",
                (
                    "twelve records describe dragon, sea-creature, volcano, and "
                    "explosion routes and sprite sequences; the map animator mutates "
                    "current position/frame, plays sounds, and gates by progression"
                ),
                "Basic.h:85-100; MenusAndIntros.h:64-85; MenusAndIntros.cpp:236-248; MenusAndIntros.cpp:4415-4608",
                confidence="STRONG",
                linked="campaign-map-screen-state",
            ),
            core_claim(
                "palette-channel-scratch",
                "rendering",
                "x_DWORD_F42A0",
                "ephemeral palette-channel arithmetic and clamp accumulator",
                (
                    "hit, flash, tint, and monochrome palette builders reassign it "
                    "for each RGB channel, clamp it to 0..63, and immediately copy "
                    "the result into the output palette"
                ),
                "EventsFunctions.cpp:3496-3498; EventsFunctions.cpp:31943-32096",
                confidence="STRONG",
            ),
            core_claim(
                "campaign-next-level-presentation",
                "rendering",
                "x_DWORD_17DE28str",
                "next-level objective text and speech presentation state machine",
                (
                    "stores phase timestamps, destination portal position, and mode "
                    "0 inactive, 1 text, 2 speech/no-text, or 3 suppressed/secret"
                ),
                "MenusAndIntros.h:53-61; MenusAndIntros.cpp:3059-3089; MenusAndIntros.cpp:3295-3329; MenusAndIntros.cpp:3578-3610",
                confidence="STRONG",
                linked="campaign-map-screen-state",
            ),
            core_claim(
                "main-menu-button-array",
                "rendering",
                "str_E1BAC",
                "sentinel-terminated main-menu button descriptors and live selection state",
                (
                    "records contain original handler address, geometry, normal and "
                    "highlight sprites, action id, and embedded dialog; runtime "
                    "updates hover, selection, availability, and dialog result"
                ),
                "MenusAndIntros.cpp:334-344; MenusAndIntros.cpp:2043-2047; MenusAndIntros.cpp:2182-2193; MenusAndIntros.cpp:5815-5877",
                confidence="STRONG",
            ),
            core_claim(
                "dynamic-help-trigger-state",
                "rendering",
                "type_str_E2A74",
                "dynamic help candidate, distance/counter, and priority state",
                (
                    "entity-backed help triggers retain a current candidate and "
                    "nearest distance, while one entry reuses the distance slot as "
                    "a persistence counter; priority ages, clamps, and is scanned "
                    "from highest to lowest"
                ),
                "GameUI.cpp:3380-3465; GameRenderHD.cpp:2175-2199; EventsFunctions.cpp:48173-48188; EventsFunctions.cpp:48892-48899; EventsFunctions.cpp:49092-49243",
                confidence="STRONG",
            ),
            core_claim(
                "bitmap-blitter-mode",
                "rendering",
                "x_WORD_E36D4",
                "legacy software bitmap-blitter mode and effect mask",
                "bitmap, text, and cursor paths branch on clipping and draw-variant bits; bit 0x40 selects colorized bitmap drawing",
                "Basic.cpp:651-659; Basic.cpp:1681-1742; Basic.cpp:2229-2300; Basic.cpp:2545-2849; PlayerInput.cpp:1689-1707",
                confidence="STRONG",
                conflicts="historical names for every bit remain unresolved",
            ),
            core_claim(
                "legacy-vga-cursor-scratch",
                "platform",
                "x_DWORD_18073A",
                "cursor blit loop and bank-selection scratch",
                "native cursor drawing uses it as an X-loop counter; banked VGA paths reuse its low word as pixel counter and high word as the current 64K bank",
                "Basic.cpp:2067-2077; EventsFunctions.cpp:51285-51449",
                confidence="STRONG",
            ),
            core_claim(
                "directional-sprite-lookup",
                "rendering",
                "GameRenderOriginal/HD/NG::x_BYTE_D4750",
                "immutable directional sprite and frame-offset lookup",
                "four slices map subtype and relative-yaw sectors into billboard rotation and frame offsets, with horizontal flip handled separately",
                "GameRenderHD.h:64-70; GameRenderHD.cpp:1893-1979; GameRenderHD.cpp:2033-2046; GameRenderHD.cpp:3641-3662; GameRenderNG.h:57-62; GameRenderOriginal.h:48-53",
                confidence="STRONG",
            ),
            core_claim(
                "texture-animation-descriptor-array",
                "rendering",
                "type_E9C08::dword_2",
                "owning pointer to fixed-capacity texture-animation descriptors",
                "allocation creates 504 descriptors, lookup scans by texture-map index, initialization fills frame metadata, and teardown frees the array",
                "Type_Animations.h:25-29; Level.cpp:1619-1649; TextureMaps.cpp:273-286; TextureMaps.cpp:700-750; EventsFunctions.cpp:45265-45273",
                confidence="STRONG",
            ),
            core_claim(
                "texture-heap-descriptor-table",
                "rendering",
                "type_x_DWORD_E9C28_str::str_8_data",
                "fixed-capacity texture-heap allocation descriptor table",
                "occupied slots track texture block, size, active order, stable descriptor index, and source texture id; storage may be inline or separately allocated",
                "engine_support.h:169-197; Level.cpp:1653-1708; TextureMaps.cpp:69-80; TextureMaps.cpp:289-340; TextureMaps.cpp:515-568; EventsFunctions.cpp:45151-45166",
                confidence="STRONG",
            ),
            core_claim(
                "level-mode-flags",
                "level",
                "type_D41A0_BYTESTR_0::byte_0x36E0B",
                "secret-level and end-sequence lock flags",
                "bit 0 marks secret/bonus-level mode and changes UI, narration, and progression; bit 1 marks the ending-sequence lock and blocks pause requests",
                "LevelInit.cpp:41-47; PlayerInput.cpp:417-426; GameUI.cpp:544-554; EventsFunctions.cpp:37297-37319; EventsFunctions.cpp:43527-43536; EventsFunctions.cpp:60473-60483",
                confidence="STRONG",
                conflicts="only bits 0 and 1 are currently proven",
            ),
            core_claim(
                "objective-presentation-state",
                "objective",
                "byte_0x36E02;byte_counter_current_objective_box_0x36E04",
                "objective narration phase and remaining textbox display time",
                "objective completion starts speech and text presentation, normally sets a 200-tick window, decrements it, and clears it for menus or close input",
                "LevelInit.cpp:40-46; PlayerInput.cpp:395-430; EventsFunctions.cpp:21999-22002; EventsFunctions.cpp:40974-41066; GameUI.cpp:679-684",
                confidence="STRONG",
                linked="objective-evaluation-order",
            ),
            core_claim(
                "end-level-statistic-denominators",
                "progression",
                "type_D41A0_BYTESTR_0::str_0x364D2",
                "eligible-creature total and collectible-spell category histogram",
                "the creature total normalizes kill percentage; the 26-entry spell array counts pickup subtypes while final percentage counts nonzero categories",
                "EventsFunctions.cpp:32958-33000; EventsFunctions.cpp:39319-39324; EventsFunctions.cpp:39870-39885; EventsFunctions.cpp:43534-43561",
                confidence="STRONG",
            ),
            core_claim(
                "player-notification-timer",
                "rendering",
                "type_str_0x2BDE::word_0x04d_2C2B_11307",
                "per-player notification remaining display ticks",
                "notification writers supply a duration; HUD rendering decrements it and clears the adjacent notification type at expiry",
                "PlayerInput.cpp:2474-2477; GameUI.cpp:4638-4642; EventsFunctions.cpp:22119-22176; EventsFunctions.cpp:37663-37671",
                confidence="STRONG",
            ),
            core_claim(
                "player-active-in-realm-flag",
                "player",
                "type_str_0x2BDE::byte_0x006_2BE4_11236",
                "per-player alive and participating-in-realm flag",
                "join/start sets it, leaving or banishment clears it, and player counting, selection, elimination, rendering, and objectives consume it",
                "EventsFunctions.cpp:22252-22320; EventsFunctions.cpp:37547-37788; EventsFunctions.cpp:40512-40520; EventsFunctions.cpp:40784-40853",
                confidence="STRONG",
            ),
            core_claim(
                "audio-volume-modal-selector",
                "rendering",
                "Type_x_D41A0_BYTEARRAY_4_struct::byte_38591",
                "active audio-volume adjustment modal",
                "zero means none, one selects sound effects, and two selects music; UI, input suppression, pause handling, and volume updates consume it",
                "Sound.cpp:6541-6557; GameUI.cpp:5165-5174; PlayerInput.cpp:626-634; PlayerInput.cpp:1751-1761; EventsFunctions.cpp:8553-8656",
                confidence="STRONG",
            ),
            core_claim(
                "frontend-pointer-snapshot",
                "platform",
                "x_DWORD_17DE38str mouse position and button fields",
                "per-menu-poll normalized pointer coordinates and button mask",
                (
                    "frontend polling copies the engine-space pointer coordinate "
                    "and low-level button latches into the aggregate; menu hit tests "
                    "consume the snapshot and clear it between frames"
                ),
                "EventsFunctions.cpp:46105-46118; MenusAndIntros.cpp:1078-1086; MenusAndIntros.cpp:1177-1178; MenusAndIntros.cpp:1597-1599",
                confidence="STRONG",
                linked="host-input-device-state;host-mouse-button-latches",
            ),
            core_claim(
                "frontend-asset-pack-source-views",
                "rendering",
                "x_DWORD_17DE48c;x_DWORD_17DE54;x_DWORD_17DEC0;x_DWORD_17DEC4",
                "frontend asset arena and mutable DAT/TAB source views",
                (
                    "an arena-relative base feeds pixel data, two table streams, "
                    "maps, palette resources, and sprite-descriptor builders; menus "
                    "can advance the base or swap alternate table sets"
                ),
                "Basic.h:110-131; Level.cpp:661-665; MenusAndIntros.cpp:742-752; MenusAndIntros.cpp:2893-2961; EventsFunctions.cpp:46132-46253",
                confidence="STRONG",
            ),
            core_claim(
                "frontend-color-remap-resource",
                "rendering",
                "x_DWORD_17DE3C",
                "frontend palette followed by color lookup and remap data",
                (
                    "the loaded resource begins with a 256-color palette and "
                    "continues with lookup/remap bytes consumed by palette fades and "
                    "indexed-pixel transformations"
                ),
                "Basic.h:50-53; MenusAndIntros.cpp:4249-4253; MenusAndIntros.cpp:4670-4674; MenusAndIntros.cpp:6238-6279; EventsFunctions.cpp:46190-46204",
                confidence="STRONG",
            ),
            core_claim(
                "normalized-engine-pointer-coordinate",
                "platform",
                "x_WORD_E3760_mouse",
                "current normalized engine-space pointer coordinate",
                (
                    "event code scales, clamps, and recenters this shared coordinate; "
                    "gameplay UI, menus, and player-input sampling consume it, so it "
                    "must not be treated as an unmodified operating-system cursor"
                ),
                "Basic.cpp:45; EventsFunctions.cpp:44488-44513; EventsFunctions.cpp:49717-49718; EventsFunctions.cpp:51513-51514; GameUI.cpp:4273-4275; GameUI.cpp:4727-4740",
                confidence="STRONG",
                linked="host-input-device-state",
            ),
            core_claim(
                "terrain-projection-tile-buffer",
                "rendering",
                "Str_E9C38_smalltit",
                "derived terrain projection, clipping, and texture working set",
                (
                    "renderers fill per-tile camera-space and screen-space points, "
                    "clip flags, billboard flags, texture indices, and UV rotations; "
                    "this is regenerated presentation state, not world terrain"
                ),
                "Type_E9C38_Smalltit.h:6-26; GameRenderNG.cpp:475-577; GameRenderHD.cpp:1249-1363",
                confidence="STRONG",
            ),
            core_claim(
                "hud-popup-textbox-state",
                "rendering",
                "str_unk_1804B0ar",
                "HUD text-box, popup, message, and score presentation state",
                (
                    "the aggregate contains two text-box layouts, popup geometry "
                    "and flags, an active UI actor index, text mode, score-display "
                    "state, and the wizard selector used by score text"
                ),
                "engine_support.h:645-718; GameUI.cpp:481-514; GameUI.cpp:3384-3441",
                confidence="STRONG",
            ),
            core_claim(
                "video-resolution-mode",
                "platform",
                "x_WORD_180660_VGA_type_resolution",
                "legacy display-resolution mode and flags",
                (
                    "selects asset variants, scaling paths, HUD layout, and world "
                    "renderer behavior; it is host/display configuration rather "
                    "than authoritative simulation state"
                ),
                "Basic.h:189-190; Level.cpp:838-1075; GameUI.cpp:481-514",
                confidence="STRONG",
            ),
            core_claim(
                "local-visual-option-blocks",
                "rendering",
                "GameSettingsStruct_t::str_0x2192;GameSettingsStruct_t::str_0x2196",
                "local smoothing, stereo, lighting, transparency, and flat-shading options",
                (
                    "the first four-byte block holds smoothing, stereoscopic "
                    "rendering, a legacy resolution mirror, and a reserved shadow "
                    "mirror; the second holds dynamic lights, a reserved byte, HUD "
                    "transparency, and flat shading"
                ),
                "LevelStructs.h:41-55; PlayerInput.cpp:187-190; PlayerInput.cpp:1460-1493; GameRenderOriginal.cpp:103-235; GameUI.cpp:2351-2762; EventsFunctions.cpp:47227-47232",
                confidence="STRONG",
                linked="video-resolution-mode;dynamic-light-registration",
            ),
            core_claim(
                "graphics-option-capability-blocks",
                "platform",
                "str_0x21AA;str_0x21AE;str_0x21B6",
                "host capability and availability gates for graphics options",
                (
                    "the blocks gate reflection, sky, shadow, dynamic light, "
                    "transparency, flat-shading, and blur controls or paths; "
                    "several reserved bytes remain unresolved"
                ),
                "Level.cpp:261-323; LevelInit.cpp:41-45; PlayerInput.cpp:1437-1492; GameUI.cpp:3943-3955; EventsFunctions.cpp:43301-43357; EventsFunctions.cpp:60581-60606",
                confidence="STRONG",
                conflicts="str_0x21AE has only its final blur-capability byte proven; other bytes remain unknown",
                linked="local-visual-option-blocks",
            ),
            core_claim(
                "stage-variable-kind-selector",
                "objective",
                "type_str_0x3647Ac::index_0x3647A_0",
                "stage-variable kind selector with authored initialization flags",
                (
                    "the authored byte stores behavior kind in its low nibble and "
                    "four initialization flags in high bits; initialization "
                    "normalizes those flags and interprets payload by kinds 1..9"
                ),
                "BasicTerrain.h:62-67; EventsFunctions.cpp:4632-4678; EventsFunctions.cpp:4726-4743; EventsFunctions.cpp:5070-5231",
                confidence="STRONG",
                linked="level-stage-variable-records",
            ),
            core_claim(
                "derived-bitmap-pixel-view",
                "rendering",
                "bitmap_pos_struct_t::data",
                "derived pointer to indexed 8-bit bitmap or sprite pixels",
                (
                    "DAT/TAB builders combine the loaded DAT base with each TAB "
                    "offset and copy dimensions into a runtime descriptor; draw and "
                    "cursor paths consume the resulting pixel stream"
                ),
                "bitmap_pos_struct.h:20-26; DatTabIndexes.cpp:3-40; DatTabIndexes.cpp:105-114; GameBitmap.cpp:223-286; PlayerInput.cpp:1666-1703",
                confidence="STRONG",
            ),
            core_claim(
                "texture-header-runtime-flags",
                "rendering",
                "type_particle_str::word_0",
                "texture header and runtime animation flags",
                (
                    "proven bits mark animated textures, textures rendered since "
                    "the last animation-service pass, and grouped animation updates; "
                    "other bits remain deliberately unresolved"
                ),
                "engine_support.h:160-167; TextureMaps.cpp:235-264; TextureMaps.cpp:493-506; TextureMaps.cpp:622-646; EventsFunctions.cpp:44981-45020",
                confidence="STRONG",
                conflicts="only bits 0, 3, and 5 are currently proven",
            ),
            core_claim(
                "software-rasterizer-mode",
                "rendering",
                "GameRenderOriginal/HD/NG::x_BYTE_E126D",
                "current polygon raster opcode and interpolation mode",
                (
                    "terrain projection selects mode 5, 7, or 26 from triangle "
                    "features; each renderer's triangle path switches on the mode "
                    "to choose interpolants and final pixel loop"
                ),
                "GameRenderOriginal.cpp:803-826; GameRenderOriginal.cpp:1375-1415; GameRenderOriginal.cpp:6722-8183; GameRenderHD.cpp:1218-1228; GameRenderNG.cpp:604-627",
                confidence="STRONG",
            ),
            core_claim(
                "texture-map-metadata-and-cache",
                "rendering",
                "str_TMAPS00TAB_BEGIN_BUFFER;str_DWORD_F66F0x",
                "loaded texture-map metadata and runtime frame-pointer cache",
                (
                    "metadata records hold data-file offsets and animation/allocation "
                    "group indices; the 504-entry pointer table and frame timestamps "
                    "are derived loading and eviction state"
                ),
                "Basic.h:263-297; TextureMaps.cpp:45-59; TextureMaps.cpp:227-265; TextureMaps.cpp:405-505",
                confidence="STRONG",
            ),
            core_claim(
                "particle-visual-parameter-table",
                "rendering",
                "particlesParameters_D951C[347]",
                "compiled sprite and particle visual parameter records",
                (
                    "records select base texture frames, visual dimensions or rates, "
                    "preload grouping, and color/shade lookup; load-time code derives "
                    "missing dimensions from texture metadata"
                ),
                "Type_WORD_D951C.h:6-19; Level.cpp:1559-1563; EventsFunctions.cpp:44934-44961; GameRenderOriginal.cpp:1959-2028",
                confidence="STRONG",
            ),
            core_claim(
                "deterministic-player-command-record",
                "determinism",
                "Type_PlayerInput_0x6E3E",
                "ten-byte per-player deterministic turn command",
                (
                    "the command contains an opcode, two opcode-dependent byte "
                    "arguments, quantized roll and pitch, a compact flag/entity "
                    "argument, and two packed entity/value arguments; multiplayer "
                    "exchange and input recording copy the exact ten bytes"
                ),
                "LevelStructs.h:151-162; EventsFunctions.cpp:37536-37590; EventsFunctions.cpp:37600-37755",
                confidence="STRONG",
                linked="deterministic-action-playback;authoritative-tick-order",
            ),
            core_claim(
                "flight-assistant-setting",
                "flight",
                "D41A0_0.byte_0x36DEA_fly_asistant",
                "local flight-assistant enable setting",
                (
                    "the setting is toggled from input, recenters and zeroes flight "
                    "commands around menus, and conditionally alters local roll and "
                    "pitch handling before command application"
                ),
                "LevelStructs.h:236-251; PlayerInput.cpp:1231-1237; PlayerInput.cpp:1988-2005; EventsFunctions.cpp:37686-37755",
                confidence="STRONG",
                linked="deterministic-player-command-record",
            ),
            core_claim(
                "host-mouse-button-latches",
                "platform",
                "x_WORD_180744_mouse_right_button;x_WORD_180746_mouse_left_button",
                "latched host mouse-button presses used by menus and input capture",
                (
                    "the latches are set on button-edge detection, consumed by "
                    "frontend and input paths, and explicitly cleared; they are host "
                    "interaction state rather than simulation state"
                ),
                "Basic.h:242-243; Animation.cpp:61-62; EventsFunctions.cpp:46108-46116; EventsFunctions.cpp:51533-51548",
                confidence="STRONG",
                linked="host-input-device-state",
            ),
            core_claim(
                "per-player-runtime-state",
                "player",
                "type_str_0x2BDE::dword_0x3E6_2BE4_12228",
                "embedded per-player flight, combat, camera, and spell state",
                (
                    "each player record embeds a full Type_str_164; the live player "
                    "entity points its auxiliary-state link at this record, while "
                    "level, input, HUD, spell, and save paths read and write it"
                ),
                "LevelStructs.h:124-132; Level.cpp:1253-1335; GameUI.cpp:3908-4131",
                confidence="STRONG",
                linked="entity-auxiliary-state-pointer;player-spell-state-block",
            ),
            core_claim(
                "live-entity-list-head",
                "runtime_entity",
                "x_D41A0_BYTEARRAY_4_struct.dword_38519",
                "head of the live-entity linked traversal list",
                (
                    "level setup points it at the first live pool record; AI, damage, "
                    "spells, castles, objectives, and world actions repeatedly follow "
                    "next_0 from this head until the pool sentinel"
                ),
                "engine_support.h:354-355; Level.cpp:334; EventsFunctions.cpp:4064-4227; EventsFunctions.cpp:27456",
                confidence="STRONG",
                linked="entity-pool-lifecycle",
            ),
            core_claim(
                "reclaimable-entity-stack-top",
                "runtime_entity",
                "type_D41A0_BYTESTR_0::dword_0x11e6",
                "top index of the secondary reclaimable-entity pointer stack",
                (
                    "level setup places eligible live entities on the secondary "
                    "stack; allocation evicts its top only when the ordinary free "
                    "stack is exhausted, and removal uses swap-with-last semantics"
                ),
                "Level.cpp:1276-1299; Events.cpp:561-607; Events.cpp:5208-5238; EventsFunctions.cpp:32905-32916",
                confidence="EXACT_STATIC",
                linked="entity-pool-lifecycle",
            ),
            core_claim(
                "entity-free-stack-top",
                "runtime_entity",
                "type_D41A0_BYTESTR_0::dword_0x35",
                "top index of the ordinary entity free-pool stack",
                "allocation pops from the stack and deletion pushes the slot back; pool initialization rebuilds it from inactive entity slots and -1 means empty",
                "Events.cpp:561-579; Events.cpp:5236-5238; Level.cpp:1281-1301; EventsFunctions.cpp:32905-32916; EventsFunctions.cpp:33262-33265",
                confidence="STRONG",
                linked="entity-pool-lifecycle;reclaimable-entity-stack-top",
            ),
            core_claim(
                "entity-polymorphic-byte-3b",
                "runtime_entity",
                "_str_0x6E8E::byte_0x3B_59",
                "class/model/action-dependent entity byte at offset 0x3B",
                "spell holders use an input-mode gate, some actors use an animation or motion preset, and other actions use a mutable counter; only contextual aliases are safe",
                "Level.cpp:1515-1523; PlayerInput.cpp:2043-2067; EventsFunctions.cpp:19797-19805; EventsFunctions.cpp:20094-20112; GameRenderOriginal.cpp:3464-3469",
                confidence="STRONG",
                linked="entity-polymorphic-payload-slots",
            ),
            core_claim(
                "ai-target-reuse-fingerprint",
                "entity_ai",
                "_str_0x6E8E::word_0x98_152",
                "cached target entity identity fingerprint",
                "target acquisition stores id + model + class shifted by seven beside the target pool index; validation recomputes it to detect recycled slots",
                "EventsFunctions.cpp:5583-5588; EventsFunctions.cpp:6111-6376; EventsFunctions.cpp:6706-6710",
                confidence="STRONG",
            ),
            core_claim(
                "entity-action-local-word-4a",
                "runtime_entity",
                "_str_0x6E8E::word_0x4A_74",
                "action-local polymorphic 16-bit scratch slot",
                (
                    "proven roles include stage selector/countdown, scripted target "
                    "entity, preserved stage variable, angular step, detached-spell "
                    "hand preference, and building-piece discriminator; no global "
                    "friendly meaning is valid"
                ),
                "EventsFunctions.cpp:4683-4722; EventsFunctions.cpp:4953-5118; EventsFunctions.cpp:10608-10646; EventsFunctions.cpp:55723-55871; EventsFunctions.cpp:62349-62365",
                confidence="EXACT_STATIC",
                linked="entity-polymorphic-payload-slots",
            ),
            core_claim(
                "pending-interaction-source",
                "runtime_entity",
                "type_str_0x5E_94::word_0x68_104",
                "pending interaction-source entity identifier",
                (
                    "area/contact writers stamp the source beside an interaction "
                    "payload; mana, creature, and building consumers use it for "
                    "ownership, claim, or reaction then clear it"
                ),
                "EventsFunctions.cpp:4161-4201; EventsFunctions.cpp:17446-17535; EventsFunctions.cpp:26015-26095; EventsFunctions.cpp:27973-28041",
                confidence="STRONG",
                linked="entity-polymorphic-payload-slots",
            ),
            core_claim(
                "derived-entity-category-lists",
                "runtime_entity",
                "dword_38523;dword_38527",
                "per-tick mana-related and building category-list heads",
                (
                    "the entity update rebuilds one list from class-10 models "
                    "39/40/57 and another from class-10 model 45; AI, mana, "
                    "placement, objectives, and destruction traverse the caches"
                ),
                "EventsFunctions.cpp:39965-40070; EventsFunctions.cpp:6517-6577; EventsFunctions.cpp:14044-14053; GameUI.cpp:3155-3180",
                confidence="EXACT_STATIC",
                linked="entity-action-dispatch",
            ),
            core_claim(
                "ai-opponent-hostility-records",
                "entity_ai",
                "Type_str_164::array_0x1FC_508",
                "per-opponent AI hostility score and forced-hostile latch records",
                (
                    "combat and mana events raise a clamped score and can latch "
                    "hostility after an aggression-scaled threshold; the score "
                    "relaxes toward a baseline and AI target selection consumes it"
                ),
                "global_types.h:294-299; EventsFunctions.cpp:5370-5393; EventsFunctions.cpp:6181-6334; EventsFunctions.cpp:7319-7430; EventsFunctions.cpp:43888-43906",
                confidence="STRONG",
                conflicts="declared uint16_t[32], but code accesses logical indices 32 and 33; the physical region extends into following storage",
            ),
            core_claim(
                "creature-attention-window",
                "entity_ai",
                "Type_str_164::word_0x248_584",
                "200-tick player creature-detection or engagement window",
                (
                    "creature acquisition and interaction paths set the timer to "
                    "200; some acquisition logic rejects players when it is zero, "
                    "and player simulation decrements it once per tick"
                ),
                "EventsFunctions.cpp:11795-11903; EventsFunctions.cpp:14170-14916; EventsFunctions.cpp:60062-60065",
                confidence="STRONG",
            ),
            core_claim(
                "locomotion-command-latch",
                "flight",
                "Type_str_164::word_0xe_14",
                "per-update locomotion-command or activity latch",
                (
                    "human and AI movement paths clear it then set it when they "
                    "command acceleration, deceleration, stopping, or obstacle "
                    "movement; pickup progress treats a set latch as movement"
                ),
                "EventsFunctions.cpp:56236-56268; EventsFunctions.cpp:60795-60839; EventsFunctions.cpp:6712-6753; EventsFunctions.cpp:7803-7806",
                confidence="STRONG",
            ),
            core_claim(
                "campaign-map-screen-state",
                "rendering",
                "x_DWORD_17DB70str",
                "campaign map-screen selection and portal animation state",
                (
                    "stores current level/portal selection, transition timing, "
                    "camera/portal positions, movement step, mode, and animation "
                    "flags for the campaign and secret-level map screens"
                ),
                "MenusAndIntros.h:32-50; MenusAndIntros.cpp:970-1020; MenusAndIntros.cpp:2899-3293",
                confidence="STRONG",
            ),
            core_claim(
                "campaign-map-level-selector",
                "rendering",
                "type_x_DWORD_17DB70str::x_WORD_17DB8A",
                "current campaign-map level or portal selector",
                (
                    "normal values select campaign portals, values above 24 route "
                    "through secret-level lookup, and -1 means no fixed portal is "
                    "selected so activation/new-game state must derive it"
                ),
                "MenusAndIntros.h:31-50; MenusAndIntros.cpp:970-997; MenusAndIntros.cpp:2967-3057; MenusAndIntros.cpp:3188-3200; MenusAndIntros.cpp:3738-3791",
                confidence="STRONG",
                linked="campaign-map-screen-state",
            ),
            core_claim(
                "legacy-ail-diagnostic-state",
                "platform",
                "x_DWORD_181BF4;x_DWORD_181BF8",
                "legacy Miles AIL debug/logging gates",
                (
                    "sound initialization clears the flags and enables them only "
                    "for the legacy debug/logging path; AIL wrapper calls use them "
                    "to guard diagnostic prefix and nested-call output"
                ),
                "Sound.cpp:1534-1568; AIL_stub.cpp:748-773; AIL_stub.cpp:841-858",
                confidence="STRONG",
            ),
        ]
    )
    for class_row in runtime_class_rows():
        rows.append(
            core_claim(
                f"runtime-class-{class_row['runtime_class']:02d}",
                "runtime_entity",
                f"class={class_row['runtime_class']}",
                str(class_row["semantic_family"]),
                "runtime entity class-family identity",
                (
                    f"{class_row['dispatch_table_anchor']}; "
                    f"{class_row['class_registry_anchor']}"
                ),
                confidence=str(class_row["confidence"]),
                conflicts=str(class_row["notes"]),
                linked="entity-runtime-selectors",
            )
        )
    for creature in creature_model_rows():
        stats = (
            f"constructor_enabled={creature['constructor_enabled']}; "
            f"actions={creature['regular_action_start']}..{creature['regular_action_end']}; "
            f"initial_action={creature['initial_action']}; "
            f"fast_cap={creature['fast_cap_raw_minSpeed']}; "
            f"slow_floor={creature['slow_floor_raw_maxSpeed']}; "
            f"life={creature['initial_life']}"
        )
        rows.append(
            core_claim(
                f"runtime-creature-model-{int(creature['runtime_model']):02d}",
                "runtime_entity",
                f"class=5;model={creature['runtime_model']}",
                str(creature["semantic_label"]),
                f"class-5 runtime creature/archetype row; {stats}",
                str(creature["evidence_anchors"]),
                confidence=str(creature["confidence"]),
                conflicts=str(creature["notes"]),
                linked="runtime-class-05;entity-constructor-dispatch;entity-action-dispatch",
            )
        )
    objective_predicates = (
        (0, "mana or castle percentage threshold", "40754-40770"),
        (1, "specified entity is dead", "40771-40778"),
        (2, "specified entity dead plus state flag", "40779-40787"),
        (3, "specified player eliminated", "40788-40794"),
        (4, "owned entity reaches target coordinates", "40795-40810"),
        (5, "player reaches target coordinates", "40811-40822"),
        (6, "collected or linked ID set contains target", "40823-40835"),
        (7, "global creature-subtype count reaches zero", "40836-40842"),
        (8, "all other players eliminated", "40843-40858"),
        (9, "no active building in linked family chain", "40859-40883"),
    )
    for objective_type, label, lines in objective_predicates:
        rows.append(
            core_claim(
                f"objective-predicate-{objective_type}",
                "objective",
                f"type={objective_type}",
                label,
                "exact completion predicate; not necessarily the user-facing title",
                f"EventsFunctions.cpp:{lines}",
                conflicts=(
                    "prose mission label still needs level text correlation"
                    if objective_type in {1, 2, 4, 6}
                    else ""
                ),
            )
        )
    return rows


def unresolved_rows() -> list[dict[str, object]]:
    return [
        {
            "issue_id": "semantic-unknown-level-subtype",
            "scope": "level_entity -> spell conversion",
            "raw_key": "GetSpellIndex_6E020 default",
            "problem": "unknown level subtype falls back to numeric family 0",
            "guardrail": "never label the defaulted source object as Fireball",
            "next_evidence": "level-record provenance plus constructor/runtime trace",
        },
        {
            "issue_id": "semantic-overloaded-entity-field-2a",
            "scope": "runtime_entity",
            "raw_key": "subSpellIndex_0x2A_42",
            "problem": "field has class/model-specific meanings",
            "guardrail": "retain neutral offset name outside proven contexts",
            "next_evidence": "per-class writer/reader partition",
        },
        {
            "issue_id": "fireball-target-specific-resolution",
            "scope": "spell/fireball",
            "raw_key": "sub_10C80 -> per-class pending-damage consumers",
            "problem": (
                "tier 0/1 payload and area writer are mapped, but target-specific "
                "shield, immunity, death, recoil, and conversion policies vary"
            ),
            "guardrail": "do not describe the raw payload as guaranteed life lost",
            "next_evidence": (
                "partition pending-damage consumers by entity class/model/action "
                "and corroborate representative targets"
            ),
        },
        {
            "issue_id": "firestorm-damage-chain",
            "scope": "spell/fireball tier 2",
            "raw_key": "family=0;tier=2;class=9 model=28;impact subtype=76",
            "problem": (
                "Fire Storm uses a distinct projectile and impact/effect chain; its "
                "loaded payload 180 is known but repetition, spread, and total damage "
                "are not yet fully reduced"
            ),
            "guardrail": "keep Fire Storm separate from ordinary Fireball/Rapid Fire",
            "next_evidence": "map class-9 model-28 and class-10 model-76 action chain",
        },
        {
            "issue_id": "creature-model-identities",
            "scope": "runtime_entity/class=5",
            "raw_key": "models 0..28",
            "problem": "many constructors have stats but no independently corroborated name",
            "guardrail": "comments remain provisional",
            "next_evidence": "level records plus INFO/manual/sprite/runtime correlation",
        },
        {
            "issue_id": "loaded-field-effect-selector",
            "scope": "spell/tier parameters",
            "raw_key": "effect_selector_raw;timing_count_raw;life_selector_raw",
            "problem": "semantics vary by spell family",
            "guardrail": "preserve raw values and assign family-specific aliases only",
            "next_evidence": "consumer partitions for each family",
        },
        {
            "issue_id": "world-scenery-model-5",
            "scope": "runtime_entity/class=2/model=5",
            "raw_key": "02:05",
            "problem": "constructor behavior exists without a defensible friendly identity",
            "guardrail": "retain raw scenery model label",
            "next_evidence": "HSPR/MSPR visual atlas and owned-level occurrence trace",
        },
        {
            "issue_id": "world-cave-command-identities",
            "scope": "authored terrain commands",
            "raw_key": "0A:53..55 and type 0E:02",
            "problem": (
                "the individual geometric operations are mapped, but the original "
                "editor-facing nouns and exact visible parameter envelopes are not"
            ),
            "guardrail": "retain raw IDs alongside behavioral labels",
            "next_evidence": "one controlled generation trace and before/after map dump per ID",
        },
        {
            "issue_id": "world-wind-vortex-force",
            "scope": "runtime_entity/class=10/model=22",
            "raw_key": "0A:16",
            "problem": "stacked vortex construction is mapped; force/influence policy is not",
            "guardrail": "do not infer global weather or wind physics from the visual entity",
            "next_evidence": "trace nearby player/creature velocity and impulse fields",
        },
        {
            "issue_id": "world-global-weather-system",
            "scope": "world simulation",
            "raw_key": "legacy type=7 subtype=4 Weather/Wind label",
            "problem": (
                "runtime class-7 tables are empty and owned levels contain no type-7 "
                "records; no global clouds, rain, or continuous weather cycle is proven"
            ),
            "guardrail": "new rich weather is successor design, not recovered MC2 behavior",
            "next_evidence": "controlled original/native play trace if weather-like behavior is observed",
        },
        {
            "issue_id": "mana-drop-fraction-shadowing",
            "scope": "creature death and mana conversion",
            "raw_key": "TransformEntityToManaSphere_36BA0 manaFraction",
            "problem": (
                "a conditional shadows the local manaFraction; literal reconstructed "
                "C++ may leave the outer value at one regardless of branch"
            ),
            "guardrail": "do not document intended mana split from source appearance alone",
            "next_evidence": "controlled original/native death and drop-state comparison",
        },
    ]


def main() -> int:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    strings = read_english_strings()
    records = read_spell_table(strings)
    owned_spell_bytes = (OWNED / "DATA/SPELLS.DAT").read_bytes()
    compiled_seed_bytes = compiled_spell_seed_bytes()
    seed_comparison = spell_seed_comparison_rows(
        owned_spell_bytes,
        compiled_seed_bytes,
    )
    icon_manifest = save_icons(records)

    tier_rows = [asdict(record) for record in records]
    family_rows = []
    for family_id, family_name in enumerate(FAMILY_NAMES):
        family_rows.append(
            {
                "family_id": family_id,
                "family_name": family_name,
                "family_display_name": strings.get(160 + family_id, family_name),
                "family_text_id": 160 + family_id,
                "family_icon_small_index": 97 + family_id,
                "family_icon_big_index": 123 + family_id,
                "runtime_spell_holder_key": f"class=15;model={family_id}",
                "confidence": "STRONG",
                "extraction_ready": True,
            }
        )
    write_csv(OUTPUT / "spell-families.csv", family_rows)
    write_csv(OUTPUT / "spell-tiers.csv", tier_rows)
    write_csv(OUTPUT / "spell-seed-vs-owned.csv", seed_comparison)
    write_csv(OUTPUT / "claims.csv", claim_rows(records))
    write_csv(OUTPUT / "core-behavior-claims.csv", core_behavior_claim_rows())
    write_csv(OUTPUT / "unresolved.csv", unresolved_rows())
    write_csv(OUTPUT / "runtime-class-families.csv", runtime_class_rows())
    write_csv(OUTPUT / "creature-models.csv", creature_model_rows())
    write_csv(OUTPUT / "creature-action-exceptions.csv", creature_action_exception_rows())
    model_annotations = runtime_model_annotation_rows(family_rows)
    write_csv(OUTPUT / "runtime-model-annotations.csv", model_annotations)
    action_annotations = runtime_action_annotation_rows()
    write_csv(OUTPUT / "runtime-action-annotations.csv", action_annotations)
    sprite_atlas_rows = save_owned_sprite_atlases()
    write_csv(OUTPUT / "owned-sprite-atlas.csv", sprite_atlas_rows)
    visual_evidence_rows = save_tmap_evidence()
    write_csv(OUTPUT / "visual-evidence.csv", visual_evidence_rows)

    asset_rows = [
        entry
        for mode_entries in icon_manifest.values()
        for entry in mode_entries
    ]
    write_csv(OUTPUT / "spell-icon-assets.csv", asset_rows)

    inputs = sorted(
        [
            OWNED / "DATA/SPELLS.DAT",
            OWNED / "LANGUAGE/D2.TXT",
            LANGUAGE_CSV,
            UPSTREAM / "remc2-editor/editor.cpp",
            UPSTREAM / "remc2/portability/InputRecorder.cpp",
            ENGINE / "BasicTerrain.h",
            ENGINE / "Compare.cpp",
            ENGINE / "Events.cpp",
            ENGINE / "EventsFunctions.cpp",
            ENGINE / "EventsFunctions.h",
            ENGINE / "GameRenderOriginal.cpp",
            ENGINE / "Spells.h",
            ENGINE / "Spells.cpp",
            ENGINE / "Level.cpp",
            ENGINE / "LevelInit.cpp",
            ENGINE / "MapType.h",
            ENGINE / "Network.cpp",
            ENGINE / "Player.cpp",
            ENGINE / "ReadAndDecompress.cpp",
            ENGINE / "SoundInGameIndexes.h",
            ENGINE / "Terrain.cpp",
            ENGINE / "Type_WORD_D951C.cpp",
            ENGINE / "WavIndexes.h",
            ENGINE / "global_types.h",
            ENGINE / "GameBitmapIndexes.h",
            HIGH_RES / "TMAPS/TMAPS2-2-424-00.data",
            HIGH_RES / "block128.data",
            *[
                OWNED / f"DATA/{name}"
                for pack, palette in MAP_MODES.values()
                for name in (f"{pack}.DAT", f"{pack}.TAB", palette)
            ],
            *[
                OWNED / f"DATA/MSPR{mode_code}0-0.{extension}"
                for mode_code in ("D", "N", "C")
                for extension in ("DAT", "TAB")
            ],
        ]
    )
    manifest = {
        "schema_version": 1,
        "classification": "PRIVATE_ANALYSIS",
        "spell_family_count": len(FAMILY_NAMES),
        "spell_tier_count": len(records),
        "runtime_class_count": len(runtime_class_rows()),
        "creature_model_count": len(creature_model_rows()),
        "creature_action_exception_count": len(creature_action_exception_rows()),
        "runtime_model_annotation_count": len(model_annotations),
        "runtime_action_annotation_count": len(action_annotations),
        "owned_sprite_atlas_binding_count": len(sprite_atlas_rows),
        "visual_evidence_count": len(visual_evidence_rows),
        "compiled_spell_seed_sha256": hashlib.sha256(
            compiled_seed_bytes
        ).hexdigest(),
        "owned_spell_table_sha256": hashlib.sha256(
            owned_spell_bytes
        ).hexdigest(),
        "owned_spell_table_equals_compiled_seed": (
            owned_spell_bytes == compiled_seed_bytes
        ),
        "spell_seed_difference_count": len(seed_comparison),
        "spell_tiers_with_seed_differences": len(
            {
                (row["family_id"], row["tier"])
                for row in seed_comparison
                if row["tier"] != ""
            }
        ),
        "claim_count": len(claim_rows(records)),
        "asset_binding_count": len(asset_rows),
        "unresolved_count": len(unresolved_rows()),
        "confidence_policy": {
            "CONFIRMED": (
                "full static lineage plus an independent causally consumed "
                "asset/data corroborator, runtime observation, or causal test"
            ),
            "EXACT_STATIC": (
                "exhaustive structural fact inside the pinned source; no runtime "
                "behavior is inferred"
            ),
            "STRONG": (
                "unambiguous linked code/data behavior; still statically inferred "
                "unless a separate runtime observation is cited"
            ),
            "PROVISIONAL": "single-source name, comment, manual, image, or adjacency",
            "CONFLICTED": "credible evidence disagrees",
        },
        "inputs": [
            {
                "path": str(path.relative_to(LAB_ROOT)),
                "bytes": path.stat().st_size,
                "sha256": sha256(path),
            }
            for path in inputs
        ],
        "outputs": {
            "spell_families": "spell-families.csv",
            "spell_tiers": "spell-tiers.csv",
            "spell_seed_vs_owned": "spell-seed-vs-owned.csv",
            "claims": "claims.csv",
            "core_behavior_claims": "core-behavior-claims.csv",
            "unresolved": "unresolved.csv",
            "runtime_class_families": "runtime-class-families.csv",
            "creature_models": "creature-models.csv",
            "creature_action_exceptions": "creature-action-exceptions.csv",
            "runtime_model_annotations": "runtime-model-annotations.csv",
            "runtime_action_annotations": "runtime-action-annotations.csv",
            "owned_sprite_atlas": "owned-sprite-atlas.csv",
            "visual_evidence": "visual-evidence.csv",
            "runtime_sprite_contact_sheets": "sprite-atlas/{day,night,cave}/{hspr,mspr}/page-*.png",
            "spell_icon_assets": "spell-icon-assets.csv",
            "day_contact_sheet": "spell-icons-day-contact.png",
            "night_contact_sheet": "spell-icons-night-contact.png",
            "cave_contact_sheet": "spell-icons-cave-contact.png",
        },
    }
    (OUTPUT / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    for directory in (SEMANTICS, OWNED, OUTPUT):
        directory.chmod(0o700)
    for path in SEMANTICS.rglob("*"):
        path.chmod(0o700 if path.is_dir() else 0o600)

    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
