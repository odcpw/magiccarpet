use sha2::{Digest, Sha256};
use std::{env, fs, path::PathBuf};

fn main() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../../upstream/magic-carpet-2-hd/remc2/utilities/Maths.cpp");
    println!("cargo:rerun-if-changed={}", source.display());
    let source_bytes = fs::read(&source).expect("read pinned private Maths.cpp");
    assert_eq!(
        format!("{:x}", Sha256::digest(&source_bytes)),
        "8bed55e5c132a2485c3258d75cb7714def2c0bd9cf17839b1bfa4b0c659d52c0",
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
            token
                .strip_prefix("0x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .unwrap_or_else(|| panic!("invalid sine-table token {token}"))
                .cast_signed()
        })
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 2_560, "sine-table denominator drift");
    assert_eq!(
        [values[0], values[512], values[1024], values[1536]],
        [0, 65_536, 0, -65_536]
    );

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("sine_table.rs");
    fs::write(
        output,
        format!("pub const SIN_16_16: [i32; 2560] = {values:?};\n"),
    )
    .expect("write generated sine table");
}
