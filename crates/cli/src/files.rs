use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
use walkdir::WalkDir;

pub fn discover(roots: &[PathBuf], extensions: &[&str], subtitles: bool) -> Result<Vec<PathBuf>> {
    let mut files = BTreeSet::new();
    for root in roots {
        if root.is_file() {
            if !matches_extension(root, extensions) {
                bail!("unsupported file extension: {}", root.display());
            }
            files.insert(
                root.canonicalize()
                    .with_context(|| format!("resolve {}", root.display()))?,
            );
        } else if root.is_dir() {
            for entry in WalkDir::new(root).follow_links(false).sort_by_file_name() {
                let entry = entry.with_context(|| format!("scan {}", root.display()))?;
                if !entry.file_type().is_file() || !matches_extension(entry.path(), extensions) {
                    continue;
                }
                if subtitles
                    && entry
                        .file_name()
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .ends_with(".assfonts.ass")
                {
                    continue;
                }
                files.insert(entry.path().canonicalize()?);
            }
        } else {
            bail!("input path is not a file or directory: {}", root.display());
        }
    }
    if files.is_empty() {
        bail!("no matching files in supplied paths");
    }
    Ok(files.into_iter().collect())
}

fn matches_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| extensions.iter().any(|ext| e.eq_ignore_ascii_case(ext)))
}

pub fn output_for(input: &Path, directory: Option<&Path>) -> Result<PathBuf> {
    let parent = directory
        .or_else(|| input.parent())
        .context("input has no parent")?;
    let mut name = input
        .file_stem()
        .context("input has no filename")?
        .to_os_string();
    name.push(".assfonts.ass");
    Ok(parent.join(name))
}

/// Resolve parent symlinks before comparing paths; refuse symlink destinations.
pub fn prepare_target(path: &Path, overwrite: bool) -> Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let target = parent
        .canonicalize()?
        .join(path.file_name().context("output needs a filename")?);
    if let Ok(meta) = fs::symlink_metadata(&target) {
        if !meta.is_file() {
            bail!(
                "output is not a regular file (symlinks are refused): {}",
                target.display()
            );
        }
        if !overwrite {
            bail!(
                "output already exists: {} (use --overwrite)",
                target.display()
            );
        }
    }
    Ok(target)
}

/// Stage in the same directory and publish atomically. The no-clobber path is
/// race-safe: a destination created after preflight is not overwritten.
pub fn atomic_write(path: &Path, bytes: &[u8], overwrite: bool) -> Result<()> {
    let mut temporary = NamedTempFile::new_in(path.parent().context("output needs parent")?)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    if overwrite {
        temporary.persist(path).map_err(|e| e.error)?;
    } else {
        temporary.persist_noclobber(path).map_err(|e| e.error)?;
    }
    Ok(())
}
