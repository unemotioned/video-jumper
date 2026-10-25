use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

fn cache_path() -> io::Result<PathBuf> {
    std::env::home_dir()
        .map(|home| home.join(".cache/vj/vj.toml"))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Could not locate home directory"))
}

#[derive(Default, Deserialize, Serialize)]
struct Status {
    #[serde(default)]
    directories: BTreeMap<String, Progress>,
}

fn read_status(path: &Path) -> io::Result<Status> {
    match fs::read_to_string(path) {
        Ok(contents) => toml::from_str(&contents)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Status::default()),
        Err(error) => Err(error),
    }
}

fn directory_key(directory: &Path) -> io::Result<String> {
    directory
        .canonicalize()?
        .into_os_string()
        .into_string()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Directory path is not valid UTF-8",
            )
        })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub(crate) struct Progress {
    pub(crate) videos: Vec<String>,
    pub(crate) seconds: f64,
}

pub(crate) fn load(directory: &Path, videos: &[String]) -> io::Result<Option<Progress>> {
    load_from(&cache_path()?, directory, videos)
}

fn load_from(path: &Path, directory: &Path, videos: &[String]) -> io::Result<Option<Progress>> {
    let Some(mut progress) = read_status(path)?
        .directories
        .remove(&directory_key(directory)?)
    else {
        return Ok(None);
    };
    if !progress.seconds.is_finite() || progress.seconds < 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid resume timestamp",
        ));
    }
    if !progress
        .videos
        .first()
        .is_some_and(|name| videos.contains(name))
    {
        return Ok(None);
    }
    progress.videos.retain(|name| videos.contains(name));
    Ok(Some(progress))
}

pub(crate) struct Recorder {
    directory: String,
    path: PathBuf,
    latest: Option<Progress>,
    saved_at: Option<Instant>,
}

impl Recorder {
    pub(crate) fn new(directory: &Path) -> io::Result<Self> {
        Self::at_path(directory, cache_path()?)
    }

    fn at_path(directory: &Path, path: PathBuf) -> io::Result<Self> {
        Ok(Self {
            directory: directory_key(directory)?,
            path,
            latest: None,
            saved_at: None,
        })
    }

    pub(crate) fn observe(&mut self, videos: &[&str], seconds: f64) -> io::Result<()> {
        if !seconds.is_finite() || seconds < 0.0 || videos.is_empty() {
            return Ok(());
        }
        let changed_video = self
            .latest
            .as_ref()
            .is_none_or(|last| last.videos.len() != videos.len() || last.videos[0] != videos[0]);
        self.latest = Some(Progress {
            videos: videos.iter().map(|name| (*name).to_owned()).collect(),
            seconds,
        });
        if changed_video
            || self
                .saved_at
                .is_none_or(|last| last.elapsed() >= Duration::from_millis(250))
        {
            self.save()?;
        }
        Ok(())
    }

    pub(crate) fn save(&mut self) -> io::Result<()> {
        if let Some(progress) = &self.latest {
            let parent = self.path.parent().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Cache path has no parent")
            })?;
            fs::create_dir_all(parent)?;
            // Lock a separate file because atomic replacement changes the cache inode.
            let lock = File::options()
                .create(true)
                .truncate(false)
                .write(true)
                .open(self.path.with_extension("lock"))?;
            lock.lock()?;
            let mut status = read_status(&self.path)?;
            status
                .directories
                .insert(self.directory.clone(), progress.clone());
            let contents = toml::to_string(&status).map_err(io::Error::other)?;
            let mut file = tempfile::NamedTempFile::new_in(parent)?;
            file.write_all(contents.as_bytes())?;
            file.flush()?;
            file.persist(&self.path)?;
            self.saved_at = Some(Instant::now());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_directory_local_and_preserves_remaining_duplicates() {
        let directory = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let videos = vec!["a.mp4".to_owned(), "b.mp4".to_owned()];
        let cache = tempfile::tempdir().unwrap();
        let path = cache.path().join(".cache/vj/vj.toml");
        let mut recorder = Recorder::at_path(directory.path(), path.clone()).unwrap();
        recorder
            .observe(&["b.mp4", "a.mp4", "b.mp4"], 42.125)
            .unwrap();
        let progress = load_from(&path, directory.path(), &videos)
            .unwrap()
            .unwrap();
        assert_eq!(progress.seconds, 42.125);
        assert_eq!(progress.videos, ["b.mp4", "a.mp4", "b.mp4"]);
        assert!(load_from(&path, other.path(), &videos).unwrap().is_none());
        let mut other_recorder = Recorder::at_path(other.path(), path.clone()).unwrap();
        other_recorder.observe(&["a.mp4"], 10.0).unwrap();
        assert!(
            load_from(&path, directory.path(), &["a.mp4".to_owned()])
                .unwrap()
                .is_none()
        );
        recorder.observe(&["a.mp4", "b.mp4"], 3.0).unwrap();
        assert_eq!(
            load_from(&path, directory.path(), &videos)
                .unwrap()
                .unwrap()
                .videos,
            ["a.mp4", "b.mp4"]
        );
        assert_eq!(
            load_from(&path, other.path(), &videos)
                .unwrap()
                .unwrap()
                .seconds,
            10.0
        );
        assert_eq!(read_status(&path).unwrap().directories.len(), 2);
    }
}
