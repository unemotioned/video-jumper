use std::{env, fs, process::Command};

const EXTENSIONS: [&str; 3] = ["mp4", "avi", "mov"];

fn read_dir() -> Vec<String> {
    let dir = env::current_dir().expect("Failed to get current directory");

    // Vec<String>: similar to List<String> in Java
    let mut files: Vec<String> = fs::read_dir(&dir)
        .expect("Failed to read directory")
        // anonymous function ex: |x| x + 2
        .filter_map(|entry| entry.ok()) // DirEntry without error
        .map(|entry| entry.file_name().to_string_lossy().into_owned()) // DirEntry into owned string
        .filter(|name| matches_video(name)) // filter specific extensions
        .collect(); // into Vec

    if files.is_empty() {
        eprintln!("No video files found");
        std::process::exit(1); // exit with error
    }

    files.sort_by(|a, b| natord::compare(a, b)); // natural order

    files
}

fn matches_video(name: &str) -> bool {
    // next_back(): last piece after split
    // is_some_and(): true if Some and closure returns true
    name.split('.')
        .next_back()
        .is_some_and(|ext| EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

fn main() {
    let videos: Vec<String> = read_dir();

    let mut vlc = Command::new("mpv")
        .arg("--speed=1.5")
        .arg("--")
        .arg(&videos[13])
        .spawn()
        .expect("Failed to launch MPV");

    vlc.wait().expect("Failed to wait for MPV");
}
