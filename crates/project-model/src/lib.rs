//! PulseCut canonical project model.
//!
//! This crate is the single source of truth for timeline state. The UI, the preview
//! renderer and the exporter all consume the same [`Project`] value; edits only happen
//! through [`EditCommand`]s applied by an [`Editor`].

pub mod commands;
pub mod history;
pub mod io;
pub mod model;
pub mod time;
pub mod validate;

pub use commands::{apply_command, EditCommand, EditError, EditOutcome, TrimEdge};
pub use history::{Editor, HistoryState};
pub use model::*;
pub use time::{format_timecode, frames_to_seconds, seconds_to_frames_floor, Rational};
