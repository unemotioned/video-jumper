use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use std::{env, fs, io, thread};

use serde_json::{Value, json};

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

fn get_property(ipc: &mut BufReader<UnixStream>, name: &str) -> io::Result<Value> {
    let request = json!({"command": ["get_property", name], "request_id": 1});
    writeln!(ipc.get_mut(), "{request}")?;
    loop {
        let mut line = String::new();
        if ipc.read_line(&mut line)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "MPV IPC closed",
            ));
        }
        let response: Value = serde_json::from_str(&line)?;
        // Events can arrive between a request and its reply.
        if response["request_id"].as_u64() == Some(1) {
            return Ok(if response["error"] == "success" {
                response["data"].clone()
            } else {
                Value::Null
            });
        }
    }
}

fn report_playback(player: &mut Child, socket: &Path) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let stream = loop {
        if player.try_wait()?.is_some() {
            return Ok(());
        }
        match UnixStream::connect(socket) {
            Ok(stream) => break stream,
            Err(error) if Instant::now() >= deadline => return Err(error),
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    };
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut ipc = BufReader::new(stream);
    while player.try_wait()?.is_none() {
        let position = get_property(&mut ipc, "time-pos")?;
        let frame = get_property(&mut ipc, "estimated-frame-number")?;
        let time = position
            .as_f64()
            .map_or_else(|| "--".to_owned(), |seconds| format!("{seconds:.3}s"));
        let frame = frame
            .as_i64()
            .map_or_else(|| "--".to_owned(), |number| number.to_string());
        print!("\rTime: {time:>14} | Frame (estimated): {frame:>12}");
        io::stdout().flush()?;
        thread::sleep(Duration::from_millis(250));
    }
    Ok(())
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

    // A private, short path avoids Unix socket path-length limits on macOS.
    let ipc_dir = tempfile::Builder::new()
        .prefix("vj-")
        .tempdir_in("/tmp")
        .expect("Failed to create MPV IPC directory");
    let socket = ipc_dir.path().join("mpv.sock");
    let mut player = Command::new("mpv")
        .arg(format!("--speed={}", speed)) // format!: put value into {}
        .arg(format!("--input-ipc-server={}", socket.display()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg("--")
        .args(indices.iter().map(|&index| &videos[index]))
        .spawn()
        .expect("Failed to launch MPV");

    let reporting = report_playback(&mut player, &socket);
    println!();
    if let Err(error) = reporting {
        // A socket closing at normal player exit is expected.
        if !matches!(
            error.kind(),
            io::ErrorKind::UnexpectedEof
                | io::ErrorKind::BrokenPipe
                | io::ErrorKind::ConnectionReset
        ) {
            eprintln!("Could not read MPV playback position: {error}");
        }
    }
    let status = player.wait().expect("Failed to wait for MPV");
    drop(ipc_dir);
    if !status.success() {
        eprintln!("MPV exited with {status}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests;
