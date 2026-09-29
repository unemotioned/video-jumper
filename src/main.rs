use std::io::Write;
use std::{env, fs, io, process::Command};

const EXTENSIONS: [&str; 4] = ["avi", "mov", "mp4", "ts"];

fn matches_video(name: &str) -> bool {
    // next_back(): last piece after split
    // is_some_and(): true if Some and closure returns true
    name.split('.')
        .next_back()
        .is_some_and(|ext| EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

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

fn main() {
    let videos = read_dir();
    let speed: f32 = 1.5;

    // iter(): yields &String for each element
    // enumerate(): wraps each item as (index, item)
    for (i, name) in videos.iter().enumerate() {
        // {:>3}: right-align index in 3 chars
        println!("{:>3}. {}", i + 1, name);
    }

    print!("\nSelect a video: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read input");

    // Vector indexing requires `usize`
    let index = match input.trim().parse::<usize>() {
        // 1..=videos.len(): builds inclusive range
        Ok(num) if (1..=videos.len()).contains(&num) => num - 1,
        _ => {
            eprintln!(
                "Invalid selection: enter a number between 1 and {}",
                videos.len()
            );
            std::process::exit(1);
        }
    };

    let mut player = Command::new("mpv")
        .arg(format!("--speed={}", speed)) // format!: put value into {}
        .arg("--")
        .arg(&videos[index])
        .spawn()
        .expect("Failed to launch MPV");

    player.wait().expect("Failed to wait for MPV");
}
