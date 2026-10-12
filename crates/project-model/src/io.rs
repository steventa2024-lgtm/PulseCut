//! `.pulsecut` project files: UTF-8 JSON, versioned, written atomically.

use crate::model::*;
use crate::validate::validate_project;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub const EXTENSION: &str = "pulsecut";
/// Refuse to parse absurdly large project files (they are JSON manifests, not media).
pub const MAX_PROJECT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum ProjectIoError {
    #[error("could not access {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("the project file is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("this project was saved by a newer PulseCut (schema {found}, this build supports {supported}); update PulseCut to open it")]
    NewerSchema { found: u32, supported: u32 },
    #[error("the project file is damaged or not a PulseCut project: {0}")]
    Invalid(String),
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> ProjectIoError + '_ {
    move |source| ProjectIoError::Io { path: path.display().to_string(), source }
}

/// Deterministic serialization (struct field order is fixed; collections keep model order).
pub fn to_json(project: &Project) -> Result<String, ProjectIoError> {
    let mut s = serde_json::to_string_pretty(project)?;
    s.push('\n');
    Ok(s)
}

/// Parses and migrates a project document, then checks structural invariants.
pub fn from_json(text: &str) -> Result<Project, ProjectIoError> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    let format = value.get("format").and_then(|v| v.as_str()).unwrap_or_default();
    if format != FILE_FORMAT {
        return Err(ProjectIoError::Invalid("missing \"format\": \"pulsecut.project\" header".into()));
    }
    let version = value.get("schemaVersion").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let value = migrate(value, version)?;
    let project: Project = serde_json::from_value(value)?;
    validate_project(&project).map_err(ProjectIoError::Invalid)?;
    Ok(project)
}

/// Schema migration entry point. Each future version adds a `vN -> vN+1` step here;
/// files are never rewritten in place until the user saves.
fn migrate(value: serde_json::Value, version: u32) -> Result<serde_json::Value, ProjectIoError> {
    match version {
        SCHEMA_VERSION => Ok(value),
        v if v > SCHEMA_VERSION => Err(ProjectIoError::NewerSchema { found: v, supported: SCHEMA_VERSION }),
        v => Err(ProjectIoError::Invalid(format!("unknown schema version {v}"))),
    }
}

/// Relative path from directory `base` to `target` (both absolute), if they share a root.
pub fn relative_path(base: &Path, target: &Path) -> Option<PathBuf> {
    let b: Vec<Component> = base.components().collect();
    let t: Vec<Component> = target.components().collect();
    if b.first() != t.first() {
        return None;
    }
    let common = b.iter().zip(&t).take_while(|(x, y)| x == y).count();
    let mut out = PathBuf::new();
    for _ in common..b.len() {
        out.push("..");
    }
    for c in &t[common..] {
        out.push(c.as_os_str());
    }
    Some(out)
}

/// Writes `project` to `path` atomically (temp file in the same directory, then rename),
/// refreshing each asset's project-relative path.
pub fn save(project: &Project, path: &Path) -> Result<Project, ProjectIoError> {
    let dir = path.parent().ok_or_else(|| ProjectIoError::Invalid("project path has no parent directory".into()))?;
    let mut doc = project.clone();
    for a in &mut doc.assets {
        a.relative_path = relative_path(dir, Path::new(&a.path)).map(|p| p.to_string_lossy().replace('\\', "/"));
    }
    validate_project(&doc).map_err(ProjectIoError::Invalid)?;
    let text = to_json(&doc)?;
    let tmp = dir.join(format!(".{}.{}.tmp", path.file_name().and_then(|n| n.to_str()).unwrap_or("project"), Uuid::new_v4()));
    let write = || -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        Ok(())
    };
    if let Err(e) = write() {
        let _ = fs::remove_file(&tmp);
        return Err(io_err(&tmp)(e));
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        io_err(path)(e)
    })?;
    Ok(doc)
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoadReport {
    /// Assets whose media was found through the project-relative path and re-pointed.
    pub relinked: Vec<AssetId>,
    /// Assets whose media file could not be found; they render as offline.
    pub offline: Vec<AssetId>,
}

/// Reads a project and resolves media paths (absolute first, then project-relative).
pub fn load(path: &Path) -> Result<(Project, LoadReport), ProjectIoError> {
    let meta = fs::metadata(path).map_err(io_err(path))?;
    if meta.len() > MAX_PROJECT_BYTES {
        return Err(ProjectIoError::Invalid("project file is larger than 64 MiB".into()));
    }
    let text = fs::read_to_string(path).map_err(io_err(path))?;
    let mut project = from_json(&text)?;
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut report = LoadReport { relinked: vec![], offline: vec![] };
    for a in &mut project.assets {
        if Path::new(&a.path).is_file() {
            continue;
        }
        let candidate = a.relative_path.as_deref().map(|r| dir.join(r)).filter(|p| p.is_file());
        match candidate {
            Some(p) => {
                a.path = p.canonicalize().unwrap_or(p).to_string_lossy().into_owned();
                report.relinked.push(a.id);
            }
            None => report.offline.push(a.id),
        }
    }
    Ok((project, report))
}
