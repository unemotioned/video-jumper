use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    #[serde(default)]
    pub videos: HashMap<String, Video>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Video {
    pub jumps: Vec<Jump>,
}

#[derive(Clone, Deserialize, PartialEq)]
#[serde(untagged)]
pub(crate) enum Jump {
    Skip(Skip),
    End(End),
}

#[derive(Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Skip {
    pub from: Position,
    pub to: Option<Position>,
    // Jump type is metadata; all skips use the same playback rules.
    #[serde(default, rename = "type")]
    pub _kind: JumpType,
}

#[derive(Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct End {
    pub end: Position,
    #[serde(default, rename = "type")]
    pub _kind: JumpType,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum JumpType {
    Intro,
    Outro,
    Ending,
    #[default]
    Normal,
}

#[derive(Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Position {
    Frame(Number),
    Time(Number),
}

#[derive(Clone, Deserialize, PartialEq)]
#[serde(try_from = "RawNumber")]
pub(crate) struct Number(f64);

// Accept both quoted and numeric TOML positions.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawNumber {
    Text(String),
    Number(f64),
}

impl TryFrom<RawNumber> for Number {
    type Error = String;

    fn try_from(raw: RawNumber) -> Result<Self, Self::Error> {
        let value = match raw {
            RawNumber::Text(text) => text.parse().map_err(|_| "Invalid position number")?,
            RawNumber::Number(value) => value,
        };
        if !value.is_finite() || value < 0.0 {
            return Err("Positions must be finite, nonnegative numbers".into());
        }
        Ok(Self(value))
    }
}

impl Position {
    fn validate(&self) -> Result<(), &'static str> {
        if let Self::Frame(value) = self
            && value.0.fract() != 0.0
        {
            return Err("frames must be whole numbers");
        }
        Ok(())
    }

    // Frame-based jumps wait until a valid media FPS is available.
    pub(crate) fn seconds(&self, fps: Option<f64>) -> Option<f64> {
        match self {
            Self::Time(value) => Some(value.0),
            Self::Frame(value) => fps
                .filter(|fps| fps.is_finite() && *fps > 0.0)
                .map(|fps| value.0 / fps),
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum Action {
    Seek(f64),
    End,
}

impl Jump {
    fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Skip(skip) => {
                // Only same-unit bounds can be compared without the video's FPS.
                let bounds = match (&skip.from, &skip.to) {
                    (Position::Frame(from), Some(Position::Frame(to)))
                    | (Position::Time(from), Some(Position::Time(to))) => Some((from.0, to.0)),
                    _ => None,
                };
                if bounds.is_some_and(|(from, to)| to <= from) {
                    return Err("jump destination must be after its start");
                }
                skip.from.validate()?;
                if let Some(to) = &skip.to {
                    to.validate()?;
                }
                Ok(())
            }
            Self::End(end) => end.end.validate(),
        }
    }

    pub(crate) fn action(&self, time: f64, fps: Option<f64>) -> Option<Action> {
        match self {
            Self::Skip(skip) => {
                let from = skip.from.seconds(fps)?;
                let Some(to) = &skip.to else {
                    return (time >= from).then_some(Action::End);
                };
                let to = to.seconds(fps)?;
                (time >= from && time < to).then_some(Action::Seek(to))
            }
            Self::End(end) => (time >= end.end.seconds(fps)?).then_some(Action::End),
        }
    }
}

pub(crate) struct LiveConfig {
    pub config: Config,
    path: PathBuf,
    observed: Result<Option<Vec<u8>>, String>,
}

impl LiveConfig {
    pub(crate) fn load(directory: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let path = directory.join(".vj.toml");
        let observed = Self::read(&path);
        let config =
            Self::parse(&observed).map_err(|error| format!("{}: {error}", path.display()))?;
        Ok(Self {
            config,
            path,
            observed,
        })
    }

    fn read(path: &Path) -> Result<Option<Vec<u8>>, String> {
        match std::fs::read(path) {
            Ok(contents) => Ok(Some(contents)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn parse(contents: &Result<Option<Vec<u8>>, String>) -> Result<Config, String> {
        match contents {
            Ok(Some(bytes)) => {
                Config::parse(std::str::from_utf8(bytes).map_err(|e| e.to_string())?)
            }
            Ok(None) => Ok(Config::default()),
            Err(error) => Err(error.clone()),
        }
    }

    pub(crate) fn refresh(&mut self) -> Result<bool, String> {
        let observed = Self::read(&self.path);
        if observed == self.observed {
            return Ok(false);
        }
        // Remember failed edits too, so each error is reported only once.
        self.observed = observed;
        self.config = Self::parse(&self.observed)
            .map_err(|error| format!("{}: {error}", self.path.display()))?;
        Ok(true)
    }
}

impl Config {
    pub(crate) fn parse(contents: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(contents).map_err(|error| error.to_string())?;
        for (name, video) in &config.videos {
            for jump in &video.jumps {
                jump.validate()
                    .map_err(|error| format!("{name}: {error}"))?;
            }
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Config, Jump, JumpType, LiveConfig};

    #[test]
    fn live_config_handles_creation_invalid_edits_replacement_and_deletion() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(".vj.toml");
        let mut live = LiveConfig::load(directory.path()).unwrap();
        assert!(live.config.videos.is_empty());
        assert!(!live.refresh().unwrap());

        let contents = "[videos.\"episode.mp4\"]\njumps = [{ end = { time = 10 } }]";
        std::fs::write(&path, contents).unwrap();
        assert!(live.refresh().unwrap());
        assert_eq!(
            live.config.videos["episode.mp4"].jumps[0].action(15.0, None),
            Some(Action::End)
        );
        assert!(!live.refresh().unwrap());

        for invalid in [
            b"[videos".as_slice(),
            b"\xff",
            b"[videos.bad]\njumps = [{ end = { time = -1 } }]",
        ] {
            std::fs::write(&path, invalid).unwrap();
            assert!(live.refresh().is_err());
            assert!(!live.refresh().unwrap());
            assert_eq!(
                live.config.videos["episode.mp4"].jumps[0].action(15.0, None),
                Some(Action::End)
            );
            assert!(LiveConfig::load(directory.path()).is_err());
        }

        let replacement = directory.path().join("replacement");
        std::fs::write(&replacement, contents.replace("10", "20")).unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        assert!(live.refresh().unwrap());
        assert_eq!(
            live.config.videos["episode.mp4"].jumps[0].action(15.0, None),
            None
        );

        std::fs::remove_file(&path).unwrap();
        assert!(live.refresh().unwrap());
        assert!(live.config.videos.is_empty());
        assert!(!live.refresh().unwrap());
    }

    #[test]
    fn jump_types_are_validated_and_default_to_normal() {
        for position in ["from", "end"] {
            for (metadata, expected) in [
                ("", JumpType::Normal),
                (", type = \"intro\"", JumpType::Intro),
                (", type = \"outro\"", JumpType::Outro),
                (", type = \"ending\"", JumpType::Ending),
                (", type = \"normal\"", JumpType::Normal),
            ] {
                let config = Config::parse(&format!(
                    "[videos.\"episode.mp4\"]\njumps = [{{ {position} = {{ time = 10 }}{metadata} }}]"
                ))
                .unwrap();
                let jump = &config.videos["episode.mp4"].jumps[0];
                let kind = match jump {
                    Jump::Skip(skip) => &skip._kind,
                    Jump::End(end) => &end._kind,
                };
                assert_eq!(kind, &expected);
                assert_eq!(jump.action(9.0, None), None);
                assert_eq!(jump.action(10.0, None), Some(Action::End));
            }
            for metadata in ["type = \"opening\"", "type = true", "is_opening = false"] {
                assert!(
                    Config::parse(&format!(
                        "[videos.\"episode.mp4\"]\njumps = [{{ {position} = {{ time = 10 }}, {metadata} }}]"
                    ))
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn from_without_to_ends_at_start() {
        let config = Config::parse(
            r#"[videos."episode.mp4"]
jumps = [{ from = { time = "12.5" }, type = "intro" }]"#,
        )
        .unwrap();
        let jump = &config.videos["episode.mp4"].jumps[0];
        assert_eq!(jump.action(12.4, None), None);
        assert_eq!(jump.action(12.5, None), Some(Action::End));
        assert_eq!(jump.action(20.0, None), Some(Action::End));
    }

    #[test]
    fn frame_without_to_waits_for_fps() {
        let config = Config::parse(
            r#"[videos."episode.mp4"]
jumps = [{ from = { frame = 300 } }]"#,
        )
        .unwrap();
        let jump = &config.videos["episode.mp4"].jumps[0];
        assert_eq!(jump.action(10.0, None), None);
        assert_eq!(jump.action(10.0, Some(0.0)), None);
        assert_eq!(jump.action(9.9, Some(30.0)), None);
        assert_eq!(jump.action(10.0, Some(30.0)), Some(Action::End));
    }

    #[test]
    fn optional_to_preserves_validation_and_bounded_seeks() {
        for jump in [
            "{ from = { frame = 1.5 } }",
            "{ from = { time = -1 } }",
            "{ from = { time = 10 }, to = { time = 5 } }",
            "{ from = { time = 10 }, to = { frame = 1.5 } }",
        ] {
            assert!(Config::parse(&format!("[videos.\"episode.mp4\"]\njumps = [{jump}]")).is_err());
        }
        let config = Config::parse(
            r#"[videos."episode.mp4"]
jumps = [{ from = { time = 10 }, to = { time = 20 } }]"#,
        )
        .unwrap();
        let jump = &config.videos["episode.mp4"].jumps[0];
        assert_eq!(jump.action(9.0, None), None);
        assert_eq!(jump.action(10.0, None), Some(Action::Seek(20.0)));
        assert_eq!(jump.action(20.0, None), None);
    }
}
