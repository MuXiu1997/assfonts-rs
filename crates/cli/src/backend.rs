use anyhow::{bail, Result};
use assfonts_core::Subsetter;

/// Compile-time plugin registry. Add a feature-gated adapter here; the pipeline
/// and ASS parser need no changes. No unstable Rust dynamic-library ABI.
pub fn create(name: &str) -> Result<Box<dyn Subsetter>> {
    match name {
        #[cfg(feature = "harfbuzz")]
        "harfbuzz" => Ok(Box::new(assfonts_harfbuzz::HarfBuzz::default())),
        _ => bail!(
            "backend {name:?} is not compiled in; available: {}",
            available().join(", ")
        ),
    }
}

pub fn available() -> Vec<&'static str> {
    vec![
        #[cfg(feature = "harfbuzz")]
        "harfbuzz",
    ]
}
