//! In-memory processing and backend contracts. No filesystem or native dependencies.
#![forbid(unsafe_code)]

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid subtitle: {0}")]
    Subtitle(String),
    #[error("unsupported input: {0}")]
    Unsupported(String),
    #[error("font resolution failed: {0}")]
    Font(String),
    #[error("subsetting failed: {0}")]
    Subset(String),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct FontRequest {
    pub family: String,
    pub weight: u16,
    pub italic: bool,
}
pub type FontUsage = BTreeMap<FontRequest, BTreeSet<char>>;

#[derive(Clone, Debug)]
pub struct FontFace {
    /// Informational label only; identity is derived from bytes and face index.
    pub source: String,
    pub data: Arc<[u8]>,
    pub index: u32,
}

#[derive(Clone, Debug)]
pub struct Attachment {
    pub name: String,
    pub data: Vec<u8>,
}

/// Implementations must reject syntax they cannot safely account for.
pub trait SubtitleCodec: Send + Sync {
    fn analyze(&self, subtitle: &str) -> Result<FontUsage>;
    /// Preserve original text and insert the supplied font attachments.
    fn embed(&self, subtitle: &str, fonts: &[Attachment]) -> Result<String>;
}

/// Match a face and verify coverage. Missing/ambiguous matches must be errors.
pub trait FontResolver: Send + Sync {
    fn resolve(&self, request: &FontRequest, characters: &BTreeSet<char>) -> Result<FontFace>;
}

/// Return a standalone SFNT, retaining names, shaping tables and requested coverage.
pub trait Subsetter: Send + Sync {
    fn name(&self) -> &str;
    fn subset(&self, face: &FontFace, characters: &BTreeSet<char>) -> Result<Vec<u8>>;
}

#[derive(Debug, Serialize)]
pub struct FontReport {
    pub requests: Vec<FontRequest>,
    pub source: String,
    pub source_sha256: String,
    pub face_index: u32,
    pub characters: String,
    pub source_bytes: usize,
    pub subset_bytes: usize,
    pub subset_sha256: String,
    pub attachment: String,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub backend: String,
    pub input_sha256: String,
    pub output_sha256: String,
    pub fonts: Vec<FontReport>,
}

#[derive(Debug)]
pub struct Processed {
    pub subtitle: String,
    pub attachments: Vec<Attachment>,
    pub report: Report,
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Adapters are injected as trait objects; neither CLI nor HarfBuzz is required.
pub struct Processor<'a> {
    pub codec: &'a dyn SubtitleCodec,
    pub resolver: &'a dyn FontResolver,
    pub subsetter: &'a dyn Subsetter,
}

impl Processor<'_> {
    pub fn process(&self, subtitle: &str) -> Result<Processed> {
        type Group = (FontFace, BTreeSet<char>, Vec<FontRequest>);
        let mut groups: BTreeMap<(String, u32), Group> = BTreeMap::new();
        for (request, characters) in self.codec.analyze(subtitle)? {
            // An empty set can represent a drawing-only font dependency.
            // Resolve it and let the subsetter retain its minimal support set.
            let face = self.resolver.resolve(&request, &characters)?;
            let key = (sha256(&face.data), face.index);
            let group = groups
                .entry(key)
                .or_insert_with(|| (face, BTreeSet::new(), Vec::new()));
            group.1.extend(characters);
            group.2.push(request);
        }
        let mut attachments = Vec::new();
        let mut fonts = Vec::new();
        for ((source_sha256, face_index), (face, characters, requests)) in groups {
            let data = self.subsetter.subset(&face, &characters)?;
            if data.is_empty() {
                return Err(Error::Subset("backend returned an empty font".into()));
            }
            let subset_sha256 = sha256(&data);
            // ASCII attachment names; never interpolate untrusted font names into ASS.
            let extension = if data.starts_with(b"OTTO") {
                "otf"
            } else {
                "ttf"
            };
            let name = format!("af_{subset_sha256}_0.{extension}");
            fonts.push(FontReport {
                requests,
                source: face.source,
                source_sha256,
                face_index,
                characters: characters.iter().collect(),
                source_bytes: face.data.len(),
                subset_bytes: data.len(),
                subset_sha256,
                attachment: name.clone(),
            });
            attachments.push(Attachment { name, data });
        }
        let output = self.codec.embed(subtitle, &attachments)?;
        let report = Report {
            schema_version: 1,
            backend: self.subsetter.name().into(),
            input_sha256: sha256(subtitle.as_bytes()),
            output_sha256: sha256(output.as_bytes()),
            fonts,
        };
        Ok(Processed {
            subtitle: output,
            attachments,
            report,
        })
    }
}
