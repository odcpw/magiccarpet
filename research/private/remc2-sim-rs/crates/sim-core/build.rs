use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::PathBuf;

fn parse_u16_hex_table(text: &str, declaration: &str) -> Vec<u16> {
    let declaration = text.find(declaration).expect("find table declaration");
    let body_start = text[declaration..]
        .find('{')
        .map(|offset| declaration + offset + 1)
        .expect("find table body");
    let body_end = text[body_start..]
        .find("};")
        .map(|offset| body_start + offset)
        .expect("find table terminator");
    text[body_start..body_end]
        .split(',')
        .filter_map(|token| {
            token.lines().rev().find_map(|line| {
                let line = line.split("//").next().unwrap_or_default().trim();
                (!line.is_empty()).then_some(line)
            })
        })
        .map(|token| {
            token
                .strip_prefix("0x")
                .and_then(|hex| u16::from_str_radix(hex, 16).ok())
                .unwrap_or_else(|| panic!("invalid u16 table token {token}"))
        })
        .collect()
}

fn parse_u8_decimal_table(text: &str, declaration: &str) -> Vec<u8> {
    let declaration = text.find(declaration).expect("find table declaration");
    let body_start = text[declaration..]
        .find('{')
        .map(|offset| declaration + offset + 1)
        .expect("find table body");
    let body_end = text[body_start..]
        .find("};")
        .map(|offset| body_start + offset)
        .expect("find table terminator");
    text[body_start..body_end]
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| {
            token
                .parse::<u8>()
                .unwrap_or_else(|_| panic!("invalid u8 table token {token}"))
        })
        .collect()
}

fn decimal_initializer(text: &str, declaration: &str) -> u32 {
    let start = text
        .find(declaration)
        .unwrap_or_else(|| panic!("find initializer declaration {declaration}"));
    let end = text[start..]
        .find(';')
        .map(|offset| start + offset)
        .expect("find initializer terminator");
    text[start..end]
        .rsplit_once('=')
        .map(|(_, value)| value.trim())
        .expect("find initializer value")
        .parse::<u32>()
        .unwrap_or_else(|_| panic!("invalid decimal initializer {declaration}"))
}

fn i32_initializer_as_i16(text: &str, declaration: &str) -> i16 {
    i16::try_from(decimal_initializer(text, declaration).cast_signed())
        .unwrap_or_else(|_| panic!("i32 initializer {declaration} does not fit i16"))
}

fn i16_initializer(text: &str, declaration: &str) -> i16 {
    u16::try_from(decimal_initializer(text, declaration))
        .unwrap_or_else(|_| panic!("i16 initializer {declaration} exceeds its source width"))
        .cast_signed()
}

fn parse_type_str_160_row(text: &str, index: usize) -> [i16; 10] {
    let declaration = text
        .find("type_str_160 str_D7BD6[157]")
        .expect("find special-settings declaration");
    let body_start = text[declaration..]
        .find('{')
        .map(|offset| declaration + offset + 1)
        .expect("find special-settings body");
    let body_end = text[body_start..]
        .find("};")
        .map(|offset| body_start + offset)
        .expect("find special-settings terminator");
    let row = text[body_start..body_end]
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
        .nth(index)
        .unwrap_or_else(|| panic!("special-settings row {index} is missing"));
    let values = row
        .trim_start()
        .trim_start_matches('{')
        .split(',')
        .take(10)
        .map(|token| {
            token
                .trim()
                .strip_prefix("0x")
                .and_then(|hex| u16::from_str_radix(hex, 16).ok())
                .unwrap_or_else(|| panic!("invalid special-settings token {token}"))
                .cast_signed()
        })
        .collect::<Vec<_>>();
    values
        .try_into()
        .unwrap_or_else(|_| panic!("special-settings row {index} has fewer than ten words"))
}

fn load_flight_source(private_root: &std::path::Path) -> ([i16; 10], [i16; 10], [i16; 8]) {
    let events_source = private_root.join("engine/EventsFunctions.cpp");
    println!("cargo:rerun-if-changed={}", events_source.display());
    let events_bytes = fs::read(&events_source).expect("read pinned private EventsFunctions.cpp");
    let events_sha256 = format!("{:x}", Sha256::digest(&events_bytes));
    assert_eq!(
        events_sha256, "3b01de678aff9f8662ef22a4790d5a47cb152de25592c3eeb42183e4783c503f",
        "pinned EventsFunctions.cpp content drift"
    );
    let events_text = String::from_utf8(events_bytes).expect("EventsFunctions.cpp is UTF-8");
    assert!(
        events_text.contains("event->dword_0xA0_160x = &str_D7BD6[104]")
            && events_text.contains("event->dword_0xA0_160x = &str_D7BD6[66]"),
        "AddPlayer_4A920 special-settings selection drift"
    );
    let flight_constants = [
        i32_initializer_as_i16(&events_text, "int speedIcrement_D4B84"),
        i32_initializer_as_i16(&events_text, "int x_DWORD_D4B88"),
        i32_initializer_as_i16(&events_text, "int x_DWORD_D4B8C"),
        i32_initializer_as_i16(&events_text, "int moveBoosStep_D4B90"),
        i16_initializer(&events_text, "__int16 x_WORD_D4BA4"),
        i32_initializer_as_i16(&events_text, "int x_DWORD_D4BA8"),
        i32_initializer_as_i16(&events_text, "int x_DWORD_D4BAC"),
        i16_initializer(&events_text, "__int16 x_WORD_D4BB0"),
    ];
    assert_eq!(
        flight_constants,
        [16, -80, 80, -4, 16, -80, 80, -4],
        "flight control constants drift"
    );

    let level_source = private_root.join("engine/Level.cpp");
    println!("cargo:rerun-if-changed={}", level_source.display());
    let level_bytes = fs::read(&level_source).expect("read pinned private Level.cpp");
    let level_sha256 = format!("{:x}", Sha256::digest(&level_bytes));
    assert_eq!(
        level_sha256, "836326264c5dfa69c61e58ef0a03f564bdb7f0fa597bce74d61b22a3830cdc17",
        "pinned Level.cpp content drift"
    );
    let level_text = String::from_utf8(level_bytes).expect("Level.cpp is UTF-8");
    let outdoor = parse_type_str_160_row(&level_text, 66);
    let cave = parse_type_str_160_row(&level_text, 104);
    assert_eq!(
        [outdoor[5], outdoor[6], outdoor[7], outdoor[8], outdoor[9]],
        [1_024, 256, -16, 256, 512],
        "outdoor player flight settings drift"
    );
    assert_eq!(
        [cave[5], cave[6], cave[7], cave[8], cave[9]],
        [3_072, 256, -8, 256, 512],
        "cave player flight settings drift"
    );
    (outdoor, cave, flight_constants)
}

fn load_spell_index_map(private_root: &std::path::Path) -> Vec<u8> {
    let source = private_root.join("engine/GameUI.cpp");
    println!("cargo:rerun-if-changed={}", source.display());
    let bytes = fs::read(&source).expect("read pinned private GameUI.cpp");
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(
        sha256, "92b5f63db493277dcdd717c4a94213aafd8a17db395e21d007829cc4eea4650f",
        "pinned GameUI.cpp content drift"
    );
    let text = String::from_utf8(bytes).expect("GameUI.cpp is UTF-8");
    let values = parse_u8_decimal_table(&text, "char spellIndex_D94FF[29]");
    assert_eq!(values.len(), 29, "spell-index map drift");
    values
}

fn write_generated(generated: &str) {
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("trig_table.rs");
    fs::write(output, generated).expect("write generated sine table");
}

fn pin_source(private_root: &std::path::Path, relative: &str, expected_sha256: &str) {
    let source = private_root.join(relative);
    println!("cargo:rerun-if-changed={}", source.display());
    let bytes = fs::read(&source).unwrap_or_else(|_| panic!("read pinned private {relative}"));
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(sha256, expected_sha256, "pinned {relative} content drift");
}

fn main() {
    let private_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../upstream/magic-carpet-2-hd/remc2");
    let source = private_root.join("utilities/Maths.cpp");
    println!("cargo:rerun-if-changed={}", source.display());
    let source_bytes = fs::read(&source).expect("read pinned private Maths.cpp");
    let actual_sha256 = format!("{:x}", Sha256::digest(&source_bytes));
    assert_eq!(
        actual_sha256, "8bed55e5c132a2485c3258d75cb7714def2c0bd9cf17839b1bfa4b0c659d52c0",
        "pinned Maths.cpp content drift"
    );
    let text = String::from_utf8(source_bytes).expect("Maths.cpp is UTF-8");
    let declaration = text
        .find("int32_t Maths::sin_DB750[2560]")
        .expect("find sine-table declaration");
    let body_start = text[declaration..]
        .find('{')
        .map(|offset| declaration + offset + 1)
        .expect("find sine-table body");
    let body_end = text[body_start..]
        .find("};")
        .map(|offset| body_start + offset)
        .expect("find sine-table terminator");
    let values = text[body_start..body_end]
        .split(',')
        .filter_map(|token| {
            let token = token.split("//").next().unwrap_or_default().trim();
            (!token.is_empty()).then_some(token)
        })
        .map(|token| {
            let value = token
                .strip_prefix("0x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .unwrap_or_else(|| panic!("invalid sine-table token {token}"));
            value.cast_signed()
        })
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 2_560, "sine-table denominator drift");
    assert_eq!(values[0], 0);
    assert_eq!(values[512], 65_536);
    assert_eq!(values[1024], 0);
    assert_eq!(values[1536], -65_536);

    let tangent_values = parse_u16_hex_table(&text, "uint16_t Maths::x_WORD_DE350[257]");
    assert_eq!(tangent_values.len(), 257, "tangent-table denominator drift");
    assert_eq!(tangent_values[0], 0);
    assert_eq!(tangent_values[256], 256);

    let sqrt_values = parse_u16_hex_table(&text, "uint16_t Maths::x_WORD_727B0[224]");
    assert_eq!(
        sqrt_values.len(),
        224,
        "square-root seed-table denominator drift"
    );
    assert!(sqrt_values[..32].iter().all(|value| *value != 0));

    pin_source(
        &private_root,
        "engine/Spells.cpp",
        "82130fd5347e5e98fa18031d35fbf99514d7d0b75ebecd2e7de2106d106f05d6",
    );
    pin_source(
        &private_root,
        "engine/Terrain.cpp",
        "de3b6f5b3fa332aea832a0a48bb13c82920bbb66a98f1a154be537b39bbde5e9",
    );
    pin_source(
        &private_root,
        "engine/Player.cpp",
        "bec2a9a6c636165a73d9b869f10b400f437c1c69a00be2cdbe113cc1b08d2d64",
    );

    let spell_index_map = load_spell_index_map(&private_root);

    let (outdoor_player_settings, cave_player_settings, flight_constants) =
        load_flight_source(&private_root);
    let [
        speed_step,
        reverse_limit,
        forward_limit,
        recoil_step,
        strafe_step,
        strafe_minimum,
        strafe_maximum,
        strafe_decay_step,
    ] = flight_constants;

    let generated = format!(
        "pub const SIN_16_16: [i32; 2560] = {values:?};\n\
         pub const LEGACY_ATAN: [u16; 257] = {tangent_values:?};\n\
         pub const LEGACY_SQRT_SEED: [u16; 224] = {sqrt_values:?};\n\
         pub const SOURCE_OUTDOOR_PLAYER_SETTINGS_INDEX: usize = 66;\n\
         pub const SOURCE_CAVE_PLAYER_SETTINGS_INDEX: usize = 104;\n\
         pub const SOURCE_OUTDOOR_PLAYER_SETTINGS: [i16; 10] = {outdoor_player_settings:?};\n\
         pub const SOURCE_CAVE_PLAYER_SETTINGS: [i16; 10] = {cave_player_settings:?};\n\
         pub const SOURCE_SPELL_INDEX_MAP: [u8; 29] = {spell_index_map:?};\n\
         pub const SOURCE_SPEED_STEP: i16 = {speed_step};\n\
         pub const SOURCE_REVERSE_LIMIT: i16 = {reverse_limit};\n\
         pub const SOURCE_FORWARD_LIMIT: i16 = {forward_limit};\n\
         pub const SOURCE_RECOIL_STEP: i16 = {recoil_step};\n\
         pub const SOURCE_STRAFE_STEP: i16 = {strafe_step};\n\
         pub const SOURCE_STRAFE_MINIMUM: i16 = {strafe_minimum};\n\
         pub const SOURCE_STRAFE_MAXIMUM: i16 = {strafe_maximum};\n\
         pub const SOURCE_STRAFE_DECAY_STEP: i16 = {strafe_decay_step};\n"
    );
    write_generated(&generated);
}
