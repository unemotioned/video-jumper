mod config;
mod playback;
mod selection;
mod videos;

use std::process::ExitCode;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::current_dir()?;
    // Validate all config entries before prompting or launching playback.
    let mut config = config::LiveConfig::load(&directory)?;
    let videos = videos::discover(&directory)?;
    if videos.is_empty() {
        return Err("No video files found".into());
    }

    let indices = selection::prompt(&videos)?;
    playback::play(
        indices.iter().map(|&index| videos[index].as_str()),
        &mut config,
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
