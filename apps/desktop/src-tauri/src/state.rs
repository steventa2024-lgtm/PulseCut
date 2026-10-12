use pulsecut_media_engine::{FfmpegTools, MediaError, PlaybackStream};
use pulsecut_project_model::{AssetId, Editor, HistoryState, Project, Rational, Sequence};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub struct Session {
    pub editor: Editor,
    pub path: Option<PathBuf>,
    pub offline: HashSet<AssetId>,
}

impl Session {
    pub fn new(project: Project) -> Self {
        Session {
            editor: Editor::new(project),
            path: None,
            offline: HashSet::new(),
        }
    }

    pub fn refresh_offline(&mut self) {
        self.offline = self
            .editor
            .project()
            .assets
            .iter()
            .filter(|a| !Path::new(&a.path).is_file())
            .map(|a| a.id)
            .collect();
    }

    pub fn snapshot(&self) -> ProjectState {
        ProjectState {
            project: self.editor.project().clone(),
            history: self.editor.history(),
            path: self.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            offline: self.offline.iter().copied().collect(),
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectState {
    pub project: Project,
    pub history: HistoryState,
    pub path: Option<String>,
    pub offline: Vec<AssetId>,
}

pub fn default_project() -> Project {
    Project::new(
        "Untitled",
        Sequence::new("Sequence 1", 1920, 1080, Rational::from_int(30), 48000),
    )
}

pub struct AppState {
    pub tools: Result<FfmpegTools, MediaError>,
    pub session: Mutex<Session>,
    pub playback: Mutex<Option<PlaybackStream>>,
    pub export_cancel: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    pub thumbnails: Mutex<HashMap<AssetId, Arc<Vec<u8>>>>,
    pub waveforms: Mutex<HashMap<AssetId, Arc<Vec<f32>>>>,
    pub autosave_dir: Option<PathBuf>,
}

impl AppState {
    pub fn tools(&self) -> Result<FfmpegTools, String> {
        self.tools.clone().map_err(|e| e.to_string())
    }
}
