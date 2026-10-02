#[cfg(windows)]
use std::fs::{File as IpcStream, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream as IpcStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::config::{Action, Config};

const PLAYBACK_SPEED: f32 = 1.5;

pub(crate) fn play<'a>(
    videos: impl IntoIterator<Item = &'a str>,
    config: &Config,
) -> io::Result<()> {
    let videos: Vec<_> = videos.into_iter().collect();
    // A private, short path avoids Unix socket path-length limits on macOS.
    #[cfg(unix)]
    let ipc_dir = tempfile::Builder::new().prefix("vj-").tempdir_in("/tmp")?;
    #[cfg(windows)]
    let ipc_dir = tempfile::Builder::new().prefix("vj-").tempdir()?;
    #[cfg(unix)]
    let socket = ipc_dir.path().join("mpv.sock");
    #[cfg(windows)]
    let socket = std::path::PathBuf::from(format!(
        r"\\.\pipe\{}-mpv",
        ipc_dir.path().file_name().unwrap().to_string_lossy()
    ));
    let mut player = Command::new("mpv")
        .arg(format!("--speed={PLAYBACK_SPEED}"))
        .arg(format!("--input-ipc-server={}", socket.display()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg("--")
        .args(&videos)
        .spawn()
        .map_err(|error| io::Error::new(error.kind(), format!("Failed to launch MPV: {error}")))?;

    let reporting = report_playback(&mut player, &socket, &videos, config);
    println!();
    if let Err(error) = reporting {
        // A socket closing at normal player exit is expected.
        if !matches!(
            error.kind(),
            io::ErrorKind::UnexpectedEof
                | io::ErrorKind::BrokenPipe
                | io::ErrorKind::ConnectionReset
        ) {
            eprintln!("Could not control MPV playback: {error}");
        }
    }
    let status = player.wait()?;
    drop(ipc_dir);
    if !status.success() {
        return Err(io::Error::other(format!("MPV exited with {status}")));
    }
    Ok(())
}

pub(crate) fn get_property(ipc: &mut BufReader<IpcStream>, name: &str) -> io::Result<Value> {
    let response = request(ipc, json!(["get_property", name]))?;
    Ok(if response["error"] == "success" {
        response["data"].clone()
    } else {
        Value::Null
    })
}

fn request(ipc: &mut BufReader<IpcStream>, command: Value) -> io::Result<Value> {
    let request = json!({"command": command, "request_id": 1});
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
            return Ok(response);
        }
    }
}

pub(crate) fn execute_action(ipc: &mut BufReader<IpcStream>, action: Action) -> io::Result<()> {
    let command = match action {
        Action::Seek(seconds) => json!(["seek", seconds, "absolute+exact"]),
        Action::End => json!(["playlist-next", "force"]),
    };
    let response = request(ipc, command)?;
    if response["error"] != "success" {
        return Err(io::Error::other(format!(
            "MPV jump failed: {}",
            response["error"]
        )));
    }
    Ok(())
}

fn report_playback(
    player: &mut Child,
    socket: &Path,
    videos: &[&str],
    config: &Config,
) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let stream = loop {
        if player.try_wait()?.is_some() {
            return Ok(());
        }
        #[cfg(unix)]
        let connection = IpcStream::connect(socket);
        #[cfg(windows)]
        let connection = OpenOptions::new().read(true).write(true).open(socket);
        match connection {
            Ok(stream) => break stream,
            Err(error) if Instant::now() >= deadline => return Err(error),
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    };
    #[cfg(unix)]
    {
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    }
    let mut ipc = BufReader::new(stream);
    let mut current_entry = None;
    let mut fired = Vec::new();
    let mut ended = false;
    while player.try_wait()?.is_none() {
        let entry = get_property(&mut ipc, "playlist-pos")?.as_u64();
        let position = get_property(&mut ipc, "time-pos")?;
        let frame = get_property(&mut ipc, "estimated-frame-number")?;
        let fps = get_property(&mut ipc, "container-fps")?.as_f64();
        // Discard a sample taken across a playlist transition.
        if entry != get_property(&mut ipc, "playlist-pos")?.as_u64() {
            continue;
        }
        let video = entry
            .and_then(|index| videos.get(index as usize))
            .and_then(|name| config.videos.get(*name));
        if current_entry != entry {
            current_entry = entry;
            fired = vec![false; video.map_or(0, |video| video.jumps.len())];
            ended = false;
        }
        if let Some(video) = video
            && let Some(seconds) = position.as_f64()
            && !ended
        {
            for (index, jump) in video.jumps.iter().enumerate() {
                if !fired[index]
                    && let Some(action) = jump.action(seconds, fps)
                {
                    ended = action == Action::End;
                    execute_action(&mut ipc, action)?;
                    fired[index] = true;
                    break;
                }
            }
        }
        let time = position
            .as_f64()
            .map_or_else(|| "--".to_owned(), |seconds| format!("{seconds:.3}s"));
        let frame = frame
            .as_i64()
            .map_or_else(|| "--".to_owned(), |number| number.to_string());
        print!("\rTime: {time:>14} | Frame (estimated): {frame:>12}");
        io::stdout().flush()?;
        thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}
