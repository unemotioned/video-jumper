mod config;
mod playback;
mod selection;
mod videos;

use std::process::ExitCode;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::current_dir()?;
    let config = config::Config::load(&directory)?;
    let videos = videos::discover(&directory)?;
    if videos.is_empty() {
        return Err("No video files found".into());
    }

    let indices = selection::prompt(&videos)?;
    playback::play(indices.iter().map(|&index| videos[index].as_str()), &config)?;
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
