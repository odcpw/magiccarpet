//! Retained MC2 language records and source font resources. Owner bytes never
//! pass through a host character encoding; tutorial sentinel streams are separate.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

const SOURCE_COMMIT: &str = "6806c7204d17220ecc0430759f784aea8ee2ddb2";
const FONT_IDS: [&str; 4] = ["font0", "font1", "font2", "hfont3"];
const RECORD_COUNT: usize = 471;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LocaleSummary {
    pub id: String,
    pub label: String,
    pub aliases: Vec<String>,
    pub non_ascii_record_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LocaleSnapshot {
    pub cache_path: String,
    pub binding_sha256: String,
    pub id: String,
    pub pack_sha256: String,
    pub text_sha256: String,
    pub font_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SourceGlyph {
    pub index: usize,
    pub offset: usize,
    pub length: usize,
    pub width: u8,
    pub height: u8,
    pub sha256: String,
}

#[derive(Clone, Debug)]
pub struct SourceFont {
    pub glyphs: Box<[SourceGlyph]>,
    pub masks: Box<[u8]>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LocaleObjective {
    pub selector: u8,
    pub text_id: String,
    pub text_sha256: String,
    pub raster_path: String,
    pub reference_x: u16,
    pub reference_y: u16,
    pub width: u16,
    pub height: u16,
    #[serde(skip)]
    pub indices_then_mask: Box<[u8]>,
}

#[derive(Clone, Debug)]
pub struct InstalledLocaleBinding {
    pub identity: LocaleSnapshot,
    pub records: Box<[Box<[u8]>]>,
    pub fonts: Arc<BTreeMap<String, SourceFont>>,
    pub objectives: Box<[LocaleObjective]>,
}

#[derive(Clone, Debug)]
pub struct InstalledLanguages {
    pub root: PathBuf,
    pub default_locale_id: String,
    pub summaries: Vec<LocaleSummary>,
    bindings: BTreeMap<String, Arc<InstalledLocaleBinding>>,
}

#[derive(Deserialize)]
struct Output {
    path: String,
    bytes: usize,
    sha256: String,
}
#[derive(Deserialize)]
struct FontRow {
    id: String,
    masks_path: String,
    glyphs: Vec<SourceGlyph>,
}
#[derive(Deserialize)]
struct RecordRow {
    id: String,
    selector: usize,
    offset: usize,
    bytes: usize,
    sha256: String,
}
#[derive(Deserialize)]
struct LocaleRow {
    id: String,
    label: String,
    aliases: Vec<String>,
    pack_sha256: String,
    records_path: String,
    prefix_path: String,
    text_sha256: String,
    records: Vec<RecordRow>,
    font_ids: Vec<String>,
    non_ascii_record_count: usize,
    objectives: Vec<LocaleObjective>,
}
#[derive(Deserialize)]
struct Manifest {
    default_locale_id: String,
    fonts: Vec<FontRow>,
    locales: Vec<LocaleRow>,
    outputs: Vec<Output>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn canonical(value: &Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|error| error.to_string())
}
fn read(root: &Path, relative: &str, maximum: u64) -> Result<Vec<u8>, String> {
    let path = Path::new(relative);
    if path.components().next().is_none()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("language resource path is not confined".into());
    }
    let mut current = root.to_path_buf();
    for part in path.components() {
        current.push(part.as_os_str());
        if fs::symlink_metadata(&current)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("language resources must use regular files".into());
        }
    }
    let metadata = fs::metadata(&current).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err("language resource exceeds its bounded file extent".into());
    }
    fs::read(current).map_err(|error| error.to_string())
}
fn payload<'a>(outputs: &'a BTreeMap<String, Vec<u8>>, path: &str) -> Result<&'a [u8], String> {
    outputs
        .get(path)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("language output is missing: {path}"))
}
fn glyph_index(byte: u8) -> Option<usize> {
    match byte {
        0 | 13 => None,
        9 | 32 => Some(33),
        10 => Some(34),
        other => Some(usize::from(other) + 1),
    }
}

impl InstalledLanguages {
    /// Build a complete candidate before publishing any retained resource.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref();
        if fs::symlink_metadata(root)
            .map_err(|error| error.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("language cache root must be a regular directory".into());
        }
        let root = root.canonicalize().map_err(|error| error.to_string())?;
        let mut value: Value =
            serde_json::from_slice(&read(&root, "manifest.json", 2 * 1024 * 1024)?)
                .map_err(|error| format!("invalid language manifest: {error}"))?;
        let binding = value
            .as_object_mut()
            .and_then(|object| object.remove("binding_sha256"))
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or("language binding identity is missing")?;
        if value["schema"] != "mc2-installed-language-cache/v1"
            || value["source_commit"] != SOURCE_COMMIT
            || value["glyph_mapping"] != "mc2-byte-plus-one-controls/v1"
            || value["language_prefix_bytes"] != 4785
            || value["language_record_count"] != RECORD_COUNT
            || digest(&canonical(&value)?) != binding
        {
            return Err("language cache binding identity mismatch".into());
        }
        let manifest: Manifest = serde_json::from_value(value)
            .map_err(|error| format!("incomplete language manifest: {error}"))?;
        if manifest.outputs.len() > 1024
            || manifest.locales.is_empty()
            || manifest.locales.len() > 64
            || manifest.fonts.len() != FONT_IDS.len()
        {
            return Err("language resource set is outside its supported bounds".into());
        }
        let mut outputs = BTreeMap::new();
        let mut total = 0usize;
        for row in &manifest.outputs {
            total = total
                .checked_add(row.bytes)
                .ok_or("language output extent overflow")?;
            if total > 32 * 1024 * 1024 {
                return Err("language resource set exceeds 32MiB".into());
            }
            let bytes = read(&root, &row.path, 4 * 1024 * 1024)?;
            if bytes.len() != row.bytes
                || digest(&bytes) != row.sha256
                || outputs.insert(row.path.clone(), bytes).is_some()
            {
                return Err(format!("language output identity mismatch: {}", row.path));
            }
        }
        let mut fonts = BTreeMap::new();
        for (expected, font) in FONT_IDS.iter().zip(&manifest.fonts) {
            let masks = payload(&outputs, &font.masks_path)?;
            if font.id != *expected
                || font.glyphs.is_empty()
                || font.glyphs.len() > 512
                || masks.iter().any(|&byte| byte > 1)
            {
                return Err("source font resource set is malformed".into());
            }
            for (index, glyph) in font.glyphs.iter().enumerate() {
                let mask = glyph
                    .offset
                    .checked_add(glyph.length)
                    .and_then(|end| masks.get(glyph.offset..end))
                    .ok_or("source glyph mask extent is invalid")?;
                if glyph.index != index
                    || glyph.length != usize::from(glyph.width) * usize::from(glyph.height)
                    || digest(mask) != glyph.sha256
                {
                    return Err(format!("source glyph {index} identity mismatch"));
                }
            }
            fonts.insert(
                font.id.clone(),
                SourceFont {
                    glyphs: font.glyphs.clone().into_boxed_slice(),
                    masks: masks.into(),
                },
            );
        }
        let fonts = Arc::new(fonts);
        let mut bindings = BTreeMap::new();
        let mut summaries = Vec::new();
        for locale in manifest.locales {
            let blob = payload(&outputs, &locale.records_path)?;
            let prefix = payload(&outputs, &locale.prefix_path)?;
            if locale.records.len() != RECORD_COUNT
                || prefix.len() != 4785
                || digest(blob) != locale.text_sha256
                || locale
                    .font_ids
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    != FONT_IDS
            {
                return Err("locale text/font resource set is incomplete".into());
            }
            let mut cursor = 0usize;
            let mut records: Vec<Box<[u8]>> = Vec::with_capacity(RECORD_COUNT);
            for (selector, row) in locale.records.iter().enumerate() {
                let end = row
                    .offset
                    .checked_add(row.bytes)
                    .ok_or("language record extent overflow")?;
                let record = blob
                    .get(row.offset..end)
                    .ok_or("language record extent is invalid")?;
                if row.selector != selector
                    || row.id != format!("text.mc2.record.{selector}")
                    || row.offset != cursor
                    || blob.get(end) != Some(&0)
                    || record.contains(&0)
                    || digest(record) != row.sha256
                {
                    return Err("language record identity mismatch".into());
                }
                for byte in record {
                    if let Some(index) = glyph_index(*byte) {
                        for font in fonts.values() {
                            if index >= font.glyphs.len() {
                                return Err(format!(
                                    "language record {selector} requires missing glyph {index}"
                                ));
                            }
                        }
                    }
                }
                records.push(record.into());
                cursor = end + 1;
            }
            if cursor != blob.len() {
                return Err("language record payload has unmatched bytes".into());
            }
            let font_identity = manifest
                .fonts
                .iter()
                .map(|font| {
                    Ok(json!({"id":font.id,"glyphs":font.glyphs,
                "masks_sha256":digest(payload(&outputs, &font.masks_path)?)}))
                })
                .collect::<Result<Vec<Value>, String>>()?;
            let identity = "mc2-locale-".to_owned()
                + &digest(&canonical(&json!({
                "pack_sha256":locale.pack_sha256,"text_sha256":locale.text_sha256,"prefix_sha256":digest(prefix),
                "glyph_mapping":"mc2-byte-plus-one-controls/v1","fonts":font_identity}))?);
            if locale.id != identity || locale.aliases.is_empty() || locale.aliases.len() > 128 {
                return Err("locale text/font binding identity mismatch".into());
            }
            let mut objectives = locale.objectives;
            if objectives
                .iter()
                .map(|row| row.selector)
                .collect::<Vec<_>>()
                != (48..=53).collect::<Vec<_>>()
            {
                return Err("current objective resource set is incomplete".into());
            }
            for objective in &mut objectives {
                let raster = payload(&outputs, &objective.raster_path)?;
                let pixels = usize::from(objective.width) * usize::from(objective.height);
                if objective.width == 0
                    || objective.width > 640
                    || objective.height == 0
                    || objective.height > 480
                    || [
                        objective.width,
                        objective.height,
                        objective.reference_x,
                        objective.reference_y,
                    ]
                    .iter()
                    .any(|v| v % 2 != 0)
                    || u32::from(objective.reference_x) + u32::from(objective.width) > 640
                    || u32::from(objective.reference_y) + u32::from(objective.height) > 480
                    || raster.len() != pixels * 2
                    || raster[pixels..].iter().any(|&byte| byte > 1)
                    || objective.text_id != format!("text.mc2.record.{}", objective.selector)
                    || objective.text_sha256 != digest(&records[usize::from(objective.selector)])
                {
                    return Err("objective text/font raster binding is malformed".into());
                }
                objective.indices_then_mask = raster.into();
            }
            summaries.push(LocaleSummary {
                id: identity.clone(),
                label: locale.label,
                aliases: locale.aliases,
                non_ascii_record_count: locale.non_ascii_record_count,
            });
            let resource = InstalledLocaleBinding {
                identity: LocaleSnapshot {
                    cache_path: root.to_string_lossy().into_owned(),
                    binding_sha256: binding.clone(),
                    id: identity.clone(),
                    pack_sha256: locale.pack_sha256,
                    text_sha256: locale.text_sha256,
                    font_ids: locale.font_ids,
                },
                records: records.into_boxed_slice(),
                fonts: Arc::clone(&fonts),
                objectives: objectives.into_boxed_slice(),
            };
            if bindings.insert(identity, Arc::new(resource)).is_some() {
                return Err("duplicate installed locale identity".into());
            }
        }
        if !bindings.contains_key(&manifest.default_locale_id) {
            return Err("installed default locale is missing".into());
        }
        Ok(Self {
            root,
            default_locale_id: manifest.default_locale_id,
            summaries,
            bindings,
        })
    }

    pub fn binding(&self, selected: Option<&str>) -> Result<Arc<InstalledLocaleBinding>, String> {
        self.bindings
            .get(selected.unwrap_or(&self.default_locale_id))
            .map(Arc::clone)
            .ok_or_else(|| "selected installed locale is unavailable".into())
    }
}
