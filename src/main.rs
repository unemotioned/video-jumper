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

fn selection_start(name: &str) -> usize {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let suffix = &stem[stem.trim_end_matches(|c: char| c.is_ascii_digit()).len()..];
    if !suffix.is_empty() && suffix.bytes().all(|digit| digit == b'0') {
        0
    } else {
        1
    }
}

fn parse_selection(input: &str, start: usize, end: usize) -> Option<Vec<usize>> {
    let mut indices = Vec::new();
    for token in input.split_whitespace() {
        let (first, last) = if let Some((left, right)) = token.split_once(':') {
            let first = if left.is_empty() {
                start
            } else {
                left.parse::<usize>().ok()?
            };
            let last = if right.is_empty() {
                end
            } else {
                right.parse::<usize>().ok()?
            };
            (first, last)
        } else {
            let number = token.parse::<usize>().ok()?;
            (number, number)
        };
        if first < start || last > end || first > last {
            return None;
        }
        indices.extend((first..=last).map(|number| number - start));
    }
    if indices.is_empty() {
        None
    } else {
        Some(indices)
    }
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
    let start = selection_start(&videos[0]);
    let end = start + videos.len() - 1;
    let speed: f32 = 1.5;

    // iter(): yields &String for each element
    // enumerate(): wraps each item as (index, item)
    for (i, name) in videos.iter().enumerate() {
        // {:>3}: right-align index in 3 chars
        println!("{:>3}. {}", i + start, name);
    }

    print!("\nSelect videos (numbers or ranges like 2:, :4, 2:4; space-separated): ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read input");

    let indices = match parse_selection(&input, start, end) {
        Some(indices) => indices,
        None => {
            eprintln!(
                "Invalid selection: enter numbers or ascending ranges between {} and {} (e.g. 2:, :4, 2:4)",
                start, end
            );
            std::process::exit(1);
        }
    };

    let mut player = Command::new("mpv")
        .arg(format!("--speed={}", speed)) // format!: put value into {}
        .arg("--")
        .args(indices.iter().map(|&index| &videos[index]))
        .spawn()
        .expect("Failed to launch MPV");

    player.wait().expect("Failed to wait for MPV");
}

#[cfg(test)]
mod tests;
