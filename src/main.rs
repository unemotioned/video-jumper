mod config;
mod playback;
mod resume;
mod selection;
mod videos;

use std::process::ExitCode;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut resume = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--continue" | "-C" => resume = true,
            "--help" | "-h" => {
                println!(
                    "Usage: vj [--continue | -C]\nSelect videos, or resume this directory's last playback with --continue (-C)."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {argument}").into()),
        }
    }
    let directory = std::env::current_dir()?;
    // Validate all config entries before prompting or launching playback.
    let mut config = config::LiveConfig::load(&directory)?;
    let videos = videos::discover(&directory)?;
    if videos.is_empty() {
        return Err("No video files found".into());
    }

    let progress = if resume {
        resume::load(&directory, &videos).unwrap_or_else(|error| {
            eprintln!("Could not load playback progress: {error}");
            None
        })
    } else {
        None
    };
    let (selected, start) = if let Some(progress) = progress {
        println!(
            "Resuming: {} at {:.3}s",
            progress.videos[0], progress.seconds
        );
        (progress.videos, Some(progress.seconds))
    } else {
        let indices = selection::prompt(&videos)?;
        (
            indices.iter().map(|&index| videos[index].clone()).collect(),
            None,
        )
    };
    playback::play(
        selected.iter().map(String::as_str),
        &mut config,
        &directory,
        start,
    )?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
