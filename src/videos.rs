use std::{fs, io, path::Path};

const EXTENSIONS: [&str; 4] = ["avi", "mov", "mp4", "ts"];

fn matches_video(name: &str) -> bool {
    name.split('.')
        .next_back()
        .is_some_and(|ext| EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

pub(crate) fn discover(directory: &Path) -> io::Result<Vec<String>> {
    let mut files: Vec<String> = fs::read_dir(directory)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| matches_video(name))
        .collect();

    files.sort_by(|a, b| natord::compare(a, b));
    Ok(files)
}
