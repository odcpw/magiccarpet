//! Package-selected original environment banks. Loading is transactional: no
//! resource is returned until the complete content tree and selectors agree.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::{Component, Path},
    sync::Arc,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const CACHE_SCHEMA: &str = "mc2-environment-material-cache/v1";

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct EnvironmentMap {
    pub id: u8,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct EnvironmentProfile {
    pub external_level: u16,
    pub internal_level: u16,
    pub map_type: EnvironmentMap,
    pub vga_resolution_selector: u8,
    pub texture_size: u8,
    pub terrain_graphics_flags: u8,
    pub palette: String,
    pub tables: String,
    pub colors: String,
    pub blocks: String,
    pub dormant_blocks16: String,
    pub sky: Option<String>,
    pub sprites: String,
    pub web: String,
    pub tmaps: String,
    pub key_color_1: u8,
    pub key_color_2: u8,
    pub campaign_package_sha256: String,
    pub owner_export_package_sha256: String,
    pub spec_commit: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentMaterialError(String);

impl fmt::Display for EnvironmentMaterialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for EnvironmentMaterialError {}
type Result<T> = std::result::Result<T, EnvironmentMaterialError>;
fn invalid(message: impl Into<String>) -> EnvironmentMaterialError {
    EnvironmentMaterialError(message.into())
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn canonical(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value).map_err(|e| invalid(e.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}
fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("environment {key} missing")))
}
fn identity(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

impl EnvironmentProfile {
    pub fn validate(&self) -> Result<()> {
        let (letter, name) = match self.map_type.id {
            0 => ("D", "Day"),
            1 => ("N", "Night"),
            2 => ("C", "Cave"),
            _ => return Err(invalid("unsupported environment map type")),
        };
        let fire = self.map_type.id == 1 && self.terrain_graphics_flags & 2 != 0;
        let display = if fire { "F" } else { letter };
        let prefix = if self.vga_resolution_selector == 1 {
            "M"
        } else {
            "H"
        };
        let blocks = if self.map_type.id == 0 {
            "BLOCK32".into()
        } else {
            format!("BL32{display}0-0")
        };
        let small = if self.map_type.id == 0 {
            "BLOCK16".into()
        } else {
            format!("BL16{display}0-0")
        };
        let sky = (self.map_type.id != 2).then(|| format!("SKY{letter}0-0"));
        if self.external_level == 0
            || self.map_type.name != name
            || self.vga_resolution_selector > 1
            || self.texture_size != 32
            || self.palette != format!("PAL{display}-0")
            || self.tables != format!("TABLES{letter}")
            || self.colors != format!("CLR{letter}-0")
            || self.blocks != blocks
            || self.dormant_blocks16 != small
            || self.sky != sky
            || self.sprites != format!("{prefix}SPR{letter}0-0")
            || self.web != format!("{prefix}WEB{letter}0-0")
            || self.tmaps != format!("TMAPS{}-0", self.map_type.id)
            || self.key_color_1 != if self.map_type.id == 1 { 0 } else { 254 }
            || self.key_color_2 != if self.map_type.id == 0 { 0 } else { 255 }
            || self.spec_commit != crate::mission_content::REMC2_SPEC_COMMIT
            || !identity(&self.campaign_package_sha256)
            || !identity(&self.owner_export_package_sha256)
        {
            return Err(invalid(
                "environment selectors differ from original source producer",
            ));
        }
        Ok(())
    }
}

/// The retained renderer inputs have one owner, shared by compatible frame
/// producers. Cave explicitly has no sky; this does not admit a cave renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentResources {
    pub profile: Option<EnvironmentProfile>,
    pub content_sha256: String,
    palette: Box<[u8]>,
    tables: Box<[u8]>,
    colors: Box<[u8]>,
    blocks: Box<[u8]>,
    sky: Option<Box<[u8]>>,
    day_exit_tables: Option<Box<[u8]>>,
    day_exit_renderer_tables: Option<Box<[u8]>>,
    source_tmaps: Option<Arc<crate::source_sprite_tmap::SourceSpriteTmapBank>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn generated(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../generated")
            .join(name)
    }
    #[test]
    fn real_cave_and_fire_night_keep_selected_banks_without_gameplay_claims() {
        let cave = EnvironmentResources::load(generated("mc-oi62-environment-level4-v2")).unwrap();
        assert!(cave.sky().is_none());
        assert_eq!(cave.profile.as_ref().unwrap().map_type.id, 2);
        let fire = EnvironmentResources::load(generated("mc-oi62-environment-level25-v2")).unwrap();
        let profile = fire.profile.as_ref().unwrap();
        assert_eq!(profile.palette, "PALF-0");
        assert_eq!(profile.blocks, "BL32F0-0");
        assert_eq!(profile.dormant_blocks16, "BL16F0-0");
        assert!(fire.sky().is_some());
        let owner = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../remc2-native-runtime/CD_Files/DATA");
        assert_eq!(fire.blocks(), fs::read(owner.join("BL32F0-0.DAT")).unwrap());
        assert_eq!(cave.colors(), fs::read(owner.join("CLRC-0.DAT")).unwrap());
        let mut wrong = profile.clone();
        wrong.palette = "PALN-0".into();
        assert!(wrong.validate().is_err());
        wrong = cave.profile.as_ref().unwrap().clone();
        wrong.sky = Some("SKYC0-0".into());
        assert!(wrong.validate().is_err());
    }
}

impl EnvironmentResources {
    pub(crate) fn retained(
        palette: Box<[u8]>,
        tables: Box<[u8]>,
        sky: Box<[u8]>,
        blocks: Box<[u8]>,
    ) -> Self {
        Self {
            profile: None,
            content_sha256: String::new(),
            palette,
            tables,
            colors: Box::default(),
            blocks,
            sky: Some(sky),
            day_exit_tables: None,
            day_exit_renderer_tables: None,
            source_tmaps: None,
        }
    }
    pub fn palette_rgb8(&self) -> &[u8] {
        &self.palette
    }
    pub fn tables(&self) -> &[u8] {
        &self.tables
    }
    pub fn colors(&self) -> &[u8] {
        &self.colors
    }
    pub fn source_tmaps(&self) -> Option<&crate::source_sprite_tmap::SourceSpriteTmapBank> {
        self.source_tmaps.as_deref()
    }
    pub fn blocks(&self) -> &[u8] {
        &self.blocks
    }
    pub fn sky(&self) -> Option<&[u8]> {
        self.sky.as_deref()
    }
    pub fn day_exit_tables(&self) -> Option<&[u8]> {
        self.day_exit_tables.as_deref()
    }

    /// Source action12 installs GTD2 at TABLES+0x4000 when phase5 expires.
    /// Immutable normal tables remain available for the next mission load.
    pub fn renderer_tables(&self, exit: crate::level_exit::LevelExitSequence) -> &[u8] {
        if exit.end_sequence && exit.phase >= 6 {
            self.day_exit_renderer_tables
                .as_deref()
                .unwrap_or(&self.tables)
        } else {
            &self.tables
        }
    }

    pub(crate) fn composite_exit(
        &self,
        current: &mut [u8],
        prior: &[u8],
        exit: crate::level_exit::LevelExitSequence,
    ) -> Result<()> {
        if self.profile.is_none() || exit.blur_mode == 0 {
            return Ok(());
        }
        if exit.blur_mode != 2 || current.len() != prior.len() {
            return Err(invalid("environment exit compositor mode/extent differs"));
        }
        let tables = self.renderer_tables(exit);
        for (pixel, &old) in current.iter_mut().zip(prior) {
            // DrawWorld_411A0 LABEL_33: high byte=current, low byte=MAIN.
            *pixel = tables[0x4000 + (usize::from(*pixel) << 8) + usize::from(old)];
        }
        Ok(())
    }

    pub fn load(root: impl AsRef<Path>) -> Result<Self> {
        if fs::symlink_metadata(root.as_ref())
            .map_err(|e| invalid(e.to_string()))?
            .file_type()
            .is_symlink()
        {
            return Err(invalid("environment cache root is a symlink"));
        }
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|e| invalid(e.to_string()))?;
        let manifest: Value = serde_json::from_slice(
            &fs::read(root.join("manifest.json")).map_err(|e| invalid(e.to_string()))?,
        )
        .map_err(|e| invalid(e.to_string()))?;
        if field(&manifest, "schema")? != CACHE_SCHEMA {
            return Err(invalid("unsupported environment cache schema"));
        }
        let mut unsigned = manifest.clone();
        unsigned
            .as_object_mut()
            .ok_or_else(|| invalid("environment manifest is not an object"))?
            .remove("manifest_payload_sha256");
        if field(&manifest, "manifest_payload_sha256")? != hash(&canonical(&unsigned)?) {
            return Err(invalid("environment metadata hash mismatch"));
        }
        let profile: EnvironmentProfile = serde_json::from_value(manifest["profile"].clone())
            .map_err(|e| invalid(e.to_string()))?;
        profile.validate()?;
        let rows = manifest["outputs"]
            .as_array()
            .ok_or_else(|| invalid("environment outputs missing"))?;
        let sources: BTreeSet<&str> = manifest["source_files"]
            .as_array()
            .ok_or_else(|| invalid("environment sources missing"))?
            .iter()
            .map(|r| field(r, "asset_id"))
            .collect::<Result<_>>()?;
        let mut products = BTreeMap::new();
        let mut identity_rows = BTreeMap::new();
        for row in rows {
            let relative = field(row, "path")?;
            let path = Path::new(relative);
            if path.is_absolute()
                || path
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
                || products.contains_key(relative)
            {
                return Err(invalid("invalid or duplicate environment output path"));
            }
            let full = root.join(path);
            if fs::symlink_metadata(&full)
                .map_err(|e| invalid(e.to_string()))?
                .file_type()
                .is_symlink()
                || !full
                    .canonicalize()
                    .map_err(|e| invalid(e.to_string()))?
                    .starts_with(&root)
            {
                return Err(invalid("environment output escapes cache"));
            }
            let bytes = fs::read(&full).map_err(|e| invalid(e.to_string()))?;
            let sha = field(row, "sha256")?;
            if row["bytes"].as_u64() != Some(bytes.len() as u64) || sha != hash(&bytes) {
                return Err(invalid(format!(
                    "environment output identity mismatch: {relative}"
                )));
            }
            let roots = row["source_asset_ids"]
                .as_array()
                .ok_or_else(|| invalid("environment source references missing"))?;
            if roots.is_empty()
                || roots.iter().any(|id| {
                    id.as_str()
                        .is_none_or(|id| !sources.contains(id.split('#').next().unwrap_or("")))
                })
            {
                return Err(invalid("environment output references unretained source"));
            }
            identity_rows.insert(relative, format!("{relative}\0{sha}\0{}\n", bytes.len()));
            products.insert(relative.to_owned(), bytes);
        }
        let actual = file_tree(&root, &root)?;
        let mut expected: BTreeSet<String> = products.keys().cloned().collect();
        expected.insert("manifest.json".into());
        if actual != expected {
            return Err(invalid("environment content tree differs"));
        }
        let content = field(&manifest, "cache_content_sha256")?;
        if content
            != hash(
                identity_rows
                    .values()
                    .cloned()
                    .collect::<String>()
                    .as_bytes(),
            )
        {
            return Err(invalid("environment content hash mismatch"));
        }
        let mut raw_products = vec![
            ("palette/source.rgb6", profile.palette.as_str()),
            ("palette/tables.u8", profile.tables.as_str()),
            ("palette/colors.u8", profile.colors.as_str()),
            ("terrain/blocks.atlas.u8", profile.blocks.as_str()),
            ("terrain/source16.dat", profile.dormant_blocks16.as_str()),
        ];
        if let Some(sky) = profile.sky.as_deref() {
            raw_products.push(("sky/source.u8", sky));
        }
        if profile.map_type.id == 0 {
            raw_products.push(("palette/day-exit-gtd2.dat", "GTD2"));
        }
        for (path, bank) in raw_products {
            let source_id = format!("owner_data:DATA/{bank}.DAT");
            let source = manifest["source_files"]
                .as_array()
                .and_then(|rows| rows.iter().find(|row| row["asset_id"] == source_id))
                .ok_or_else(|| invalid("selected environment source missing"))?;
            let output = rows
                .iter()
                .find(|row| row["path"] == path)
                .ok_or_else(|| invalid("selected environment output missing"))?;
            let bytes = product(&products, path, None)?;
            if output["source_asset_ids"] != serde_json::json!([source_id])
                || field(source, "content_address")? != format!("sha256:{}", hash(bytes))
                || source["size"].as_u64() != Some(bytes.len() as u64)
            {
                return Err(invalid(format!(
                    "selected owner bank identity differs: {bank}"
                )));
            }
        }
        let descriptor: Value =
            serde_json::from_slice(product(&products, "materials/descriptor.json", None)?)
                .map_err(|e| invalid(e.to_string()))?;
        if field(&descriptor, "schema")? != "mc2-environment-materials/v1"
            || descriptor["profile"] != manifest["profile"]
        {
            return Err(invalid("environment descriptor identity mismatch"));
        }
        for (section, key, path) in [
            ("palette", "path", "palette/source.rgb6"),
            ("palette", "rgb8_path", "palette/source.rgb8"),
            ("tables", "path", "palette/tables.u8"),
            ("colors", "path", "palette/colors.u8"),
            ("blocks", "atlas_path", "terrain/blocks.atlas.u8"),
            ("source16", "path", "terrain/source16.dat"),
        ] {
            if descriptor[section][key] != path {
                return Err(invalid("environment descriptor resource path differs"));
            }
        }
        if descriptor["tables"]["key_color_1"] != profile.key_color_1
            || descriptor["tables"]["key_color_2"] != profile.key_color_2
            || descriptor["blocks"]["width"] != 256
            || descriptor["blocks"]["height"] != 608
            || descriptor["blocks"]["tile_size"] != 32
            || descriptor["blocks"]["tile_count"] != 152
            || descriptor["tmaps"]["bank"] != profile.tmaps
            || (profile.sky.is_some()
                && (descriptor["sky"]["path"] != "sky/source.u8"
                    || descriptor["sky"]["width"] != 256
                    || descriptor["sky"]["height"] != 256))
        {
            return Err(invalid("environment descriptor resource selectors differ"));
        }
        let bitmap_banks = descriptor["bitmap_banks"]
            .as_array()
            .ok_or_else(|| invalid("environment bitmap banks missing"))?;
        let names: BTreeSet<&str> = bitmap_banks
            .iter()
            .map(|bank| field(bank, "bank"))
            .collect::<Result<_>>()?;
        if bitmap_banks.len() != 3
            || names != BTreeSet::from([profile.sprites.as_str(), profile.web.as_str(), "FONT1"])
        {
            return Err(invalid(
                "environment bitmap banks differ from selected sources",
            ));
        }
        let closure: Value =
            serde_json::from_slice(product(&products, "materials/closure.json", None)?)
                .map_err(|e| invalid(e.to_string()))?;
        let mut unsigned_closure = closure.clone();
        unsigned_closure
            .as_object_mut()
            .ok_or_else(|| invalid("environment closure missing"))?
            .remove("closure_sha256");
        // The closure tool's canonical encoding does not include a newline.
        let closure_bytes =
            serde_json::to_vec(&unsigned_closure).map_err(|e| invalid(e.to_string()))?;
        if closure["profile"] != manifest["profile"]
            || field(&closure, "closure_sha256")? != field(&manifest, "closure_sha256")?
            || field(&closure, "closure_sha256")? != hash(&closure_bytes)
        {
            return Err(invalid("environment closure identity mismatch"));
        }
        let rgb6 = product(&products, "palette/source.rgb6", Some(768))?;
        let palette = product(&products, "palette/source.rgb8", Some(768))?;
        if rgb6
            .iter()
            .zip(palette)
            .any(|(&six, &eight)| six > 63 || eight != six * 4)
        {
            return Err(invalid("environment palette conversion differs"));
        }
        validate_textures(&products, &descriptor)?;
        let sky = if profile.sky.is_some() {
            Some(product(&products, "sky/source.u8", Some(65536))?.into())
        } else {
            if !descriptor["sky"].is_null() || products.contains_key("sky/source.u8") {
                return Err(invalid("Cave cache must not contain a sky"));
            }
            None
        };
        let day_exit_tables = if profile.map_type.id == 0 {
            Some(product(&products, "palette/day-exit-gtd2.dat", Some(65536))?.into())
        } else {
            None
        };
        let day_exit_renderer_tables = day_exit_tables.as_deref().map(|exit: &[u8]| {
            let mut tables = products["palette/tables.u8"].clone();
            tables[0x4000..0x14000].copy_from_slice(exit);
            tables.into_boxed_slice()
        });
        // Texture payloads have already passed the complete manifest and
        // decoded owner-byte checks above. Retain their canonical bank once
        // so every source world pass shares it without per-frame file I/O.
        let frames = descriptor["tmaps"]["textures"]
            .as_array()
            .ok_or_else(|| invalid("environment textures missing"))?
            .iter()
            .filter(|row| row["source_empty"] != true)
            .map(|row| {
                Ok(crate::source_sprite_tmap::SourceSpriteTmapFrame {
                    selector: row["index"]
                        .as_u64()
                        .ok_or_else(|| invalid("TMAP selector missing"))?
                        as u16,
                    width: row["width"]
                        .as_u64()
                        .ok_or_else(|| invalid("TMAP width missing"))?
                        as u16,
                    height: row["height"]
                        .as_u64()
                        .ok_or_else(|| invalid("TMAP height missing"))?
                        as u16,
                    pixels: product(&products, field(row, "indices_path")?, None)?.into(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let metadata = descriptor["tmaps"]["textures"].as_array().unwrap().iter().map(|row| {
            let selector = u16::try_from(row["index"].as_u64().ok_or_else(|| invalid("TMAP selector missing"))?)
                .map_err(|_| invalid("TMAP selector exceeds u16"))?;
            let group = u16::try_from(row["group"].as_u64().ok_or_else(|| invalid("TMAP group missing"))?)
                .map_err(|_| invalid("TMAP group exceeds u16"))?;
            let decoded = product(&products, field(row, "decoded_payload_path")?, None)?;
            let metadata = crate::source_animation::SourceTmapMetadata::from_decoded(selector, group, &decoded)
                .map_err(invalid)?;
            if row["animated"].as_bool().unwrap_or(false) != metadata.encoded_frame_count.is_some()
                || (metadata.encoded_frame_count.is_some()
                    && row["frame_count"].as_u64() != metadata.encoded_frame_count.map(u64::from)) {
                return Err(invalid("TMAP animation metadata differs from decoded owner header"));
            }
            Ok(metadata)
        }).collect::<Result<Vec<_>>>()?;
        let catalog = crate::source_animation::SourceTmapCatalog::new(metadata).map_err(invalid)?;
        let source_tmaps = crate::source_sprite_tmap::SourceSpriteTmapBank::new(frames)
            .map_err(invalid)?.with_catalog(catalog);
        Ok(Self {
            profile: Some(profile),
            content_sha256: content.to_owned(),
            palette: palette.into(),
            tables: product(&products, "palette/tables.u8", Some(83456))?.into(),
            colors: product(&products, "palette/colors.u8", Some(4096))?.into(),
            blocks: product(&products, "terrain/blocks.atlas.u8", Some(155648))?.into(),
            sky,
            day_exit_tables,
            day_exit_renderer_tables,
            source_tmaps: Some(Arc::new(source_tmaps)),
        })
    }
}

fn product<'a>(
    products: &'a BTreeMap<String, Vec<u8>>,
    path: &str,
    extent: Option<usize>,
) -> Result<&'a [u8]> {
    let bytes = products
        .get(path)
        .ok_or_else(|| invalid(format!("environment product missing: {path}")))?;
    if extent.is_some_and(|n| n != bytes.len()) {
        return Err(invalid(format!(
            "environment product extent differs: {path}"
        )));
    }
    Ok(bytes)
}

fn file_tree(root: &Path, current: &Path) -> Result<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for entry in fs::read_dir(current).map_err(|e| invalid(e.to_string()))? {
        let entry = entry.map_err(|e| invalid(e.to_string()))?;
        let kind = entry.file_type().map_err(|e| invalid(e.to_string()))?;
        if kind.is_symlink() {
            return Err(invalid("environment cache contains a symlink"));
        }
        if kind.is_dir() {
            result.extend(file_tree(root, &entry.path())?);
        } else if kind.is_file() {
            result.insert(
                entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|e| invalid(e.to_string()))?
                    .to_str()
                    .ok_or_else(|| invalid("non UTF8 environment path"))?
                    .to_owned(),
            );
        }
    }
    Ok(result)
}

fn validate_textures(products: &BTreeMap<String, Vec<u8>>, descriptor: &Value) -> Result<()> {
    let rows = descriptor["tmaps"]["textures"]
        .as_array()
        .ok_or_else(|| invalid("environment textures missing"))?;
    if rows.len() != 504 {
        return Err(invalid("environment TMAP table is incomplete"));
    }
    for (index, row) in rows.iter().enumerate() {
        if row["index"].as_u64() != Some(index as u64) {
            return Err(invalid("environment TMAP order differs"));
        }
        let decoded = product(products, field(row, "decoded_payload_path")?, None)?;
        if row["source_empty"] == true {
            if decoded != b"0" || !row["indices_path"].is_null() {
                return Err(invalid("environment empty TMAP differs"));
            }
            continue;
        }
        if decoded.len() < 6 {
            return Err(invalid("environment TMAP header truncated"));
        }
        let width = usize::from(u16::from_le_bytes([decoded[2], decoded[3]]));
        let height = usize::from(u16::from_le_bytes([decoded[4], decoded[5]]));
        let extent = width * height;
        if extent == 0
            || decoded.len() < 6 + extent
            || row["width"].as_u64() != Some(width as u64)
            || row["height"].as_u64() != Some(height as u64)
            || product(products, field(row, "indices_path")?, Some(extent))?
                != &decoded[6..6 + extent]
        {
            return Err(invalid("environment TMAP bounds/pixels differ"));
        }
    }
    for bank in descriptor["bitmap_banks"]
        .as_array()
        .ok_or_else(|| invalid("environment bitmap banks missing"))?
    {
        for (selector, row) in bank["bitmaps"]
            .as_array()
            .ok_or_else(|| invalid("environment bitmap records missing"))?
            .iter()
            .enumerate()
        {
            if row["selector"].as_u64() != Some(selector as u64) {
                return Err(invalid("environment bitmap order differs"));
            }
            if row["source_empty"] == true {
                if row["width"].as_u64().is_none_or(|n| n > 0)
                    && row["height"].as_u64().is_none_or(|n| n > 0)
                {
                    return Err(invalid("nonempty environment bitmap marked empty"));
                }
                continue;
            }
            let extent = row["width"]
                .as_u64()
                .and_then(|w| row["height"].as_u64().and_then(|h| w.checked_mul(h)))
                .and_then(|n| usize::try_from(n).ok())
                .filter(|&n| n > 0)
                .ok_or_else(|| invalid("environment bitmap dimensions invalid"))?;
            product(products, field(row, "indices_path")?, Some(extent))?;
            if product(products, field(row, "mask_path")?, Some(extent))?
                .iter()
                .any(|&n| n > 1)
            {
                return Err(invalid("environment coverage is not a binary mask"));
            }
        }
    }
    Ok(())
}
