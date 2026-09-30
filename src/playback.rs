use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const PLAYBACK_SPEED: f32 = 1.5;

pub(crate) fn play<'a>(videos: impl IntoIterator<Item = &'a str>) -> io::Result<()> {
    // A private, short path avoids Unix socket path-length limits on macOS.
    let ipc_dir = tempfile::Builder::new().prefix("vj-").tempdir_in("/tmp")?;
    let socket = ipc_dir.path().join("mpv.sock");
    let mut player = Command::new("mpv")
        .arg(format!("--speed={PLAYBACK_SPEED}"))
        .arg(format!("--input-ipc-server={}", socket.display()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg("--")
        .args(videos)
        .spawn()
        .map_err(|error| io::Error::new(error.kind(), format!("Failed to launch MPV: {error}")))?;

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
    let status = player.wait()?;
    drop(ipc_dir);
    if !status.success() {
        return Err(io::Error::other(format!("MPV exited with {status}")));
    }
    Ok(())
}

pub(crate) fn get_property(ipc: &mut BufReader<UnixStream>, name: &str) -> io::Result<Value> {
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
