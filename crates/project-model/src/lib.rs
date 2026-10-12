//! Canonical PulseCut project document.
//!
//! The UI, the preview renderer, and the exporter all read this model.
//! Edits go through [`Session::dispatch`] so undo history and the saved file
//! describe the same timeline.

mod edit;
mod error;
mod model;
mod time;

pub use edit::{Command, Session, SessionSnapshot, TrimEdge};
pub use error::ProjectError;
pub use model::{
    canonical_json, from_json, new_id, prepare_for_save, resolve_after_load, to_json_pretty,
    validate_media_path, validate_structure, Asset, Clip, Project, Sequence, Track, TrackKind,
    SCHEMA_VERSION,
};
pub use time::{format_seconds, Rational};
