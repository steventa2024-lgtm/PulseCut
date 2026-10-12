//! Locating and invoking the external FFmpeg/ffprobe executables.
//!
//! FFmpeg runs as a separate process (never linked), so PulseCut can use a
//! user-supplied or separately-distributed FFmpeg build. Arguments are always built
//! by this crate from validated model data; nothing from the UI is passed through.

use crate::MediaError;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct FfmpegTools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub version: String,
    pub encoders: HashSet<String>,
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// A `Command` that never opens a console window on Windows.
pub fn command(program: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdin(Stdio::null());
    cmd
}

/// FFmpeg stderr with dynamic-loader warnings removed. glibc prints
/// "no version information available" when `LD_LIBRARY_PATH` shadows a system
/// library; it is not an FFmpeg error and must not fail playback or verification.
pub fn ffmpeg_diagnostics(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.contains("no version information available"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn candidates() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(d) = std::env::var_os("PULSECUT_FFMPEG_DIR") {
        dirs.push(PathBuf::from(d));
    }
    if let Ok(cur) = std::env::current_exe() {
        if let Some(d) = cur.parent() {
            dirs.push(d.join("ffmpeg"));
            dirs.push(d.to_path_buf());
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs
}

impl FfmpegTools {
    /// Finds ffmpeg + ffprobe: `$PULSECUT_FFMPEG_DIR`, then next to the executable, then `PATH`.
    pub fn locate() -> Result<Self, MediaError> {
        for dir in candidates() {
            let (ff, fp) = (dir.join(exe("ffmpeg")), dir.join(exe("ffprobe")));
            if ff.is_file() && fp.is_file() {
                return Self::from_paths(ff, fp);
            }
        }
        Err(MediaError::ToolMissing)
    }

    pub fn from_paths(ffmpeg: PathBuf, ffprobe: PathBuf) -> Result<Self, MediaError> {
        let out = command(&ffmpeg)
            .args(["-hide_banner", "-version"])
            .output()
            .map_err(|_| MediaError::ToolMissing)?;
        let version = String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .to_string();
        let enc = command(&ffmpeg)
            .args(["-hide_banner", "-encoders"])
            .output()
            .map_err(|_| MediaError::ToolMissing)?;
        let encoders = String::from_utf8_lossy(&enc.stdout)
            .lines()
            .filter_map(|l| {
                let mut parts = l.split_whitespace();
                let flags = parts.next()?;
                (flags.len() == 6 && !flags.contains('='))
                    .then(|| parts.next().map(str::to_string))
                    .flatten()
            })
            .collect();
        Ok(FfmpegTools {
            ffmpeg,
            ffprobe,
            version,
            encoders,
        })
    }

    pub fn has_encoder(&self, name: &str) -> bool {
        self.encoders.contains(name)
    }
}

#[cfg(test)]
mod tests {
    use super::ffmpeg_diagnostics;

    #[test]
    fn loader_warnings_are_not_diagnostics() {
        let err = "ffmpeg: /opt/lib/libncursesw.so.6: no version information available (required by /lib/libcaca.so.0)\n\n";
        assert_eq!(ffmpeg_diagnostics(err), "");
        let real = format!("{err}[h264 @ 0x1] corrupt macroblock\n");
        assert_eq!(ffmpeg_diagnostics(&real), "[h264 @ 0x1] corrupt macroblock");
    }
}
