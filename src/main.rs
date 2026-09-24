use std::process::Command;

fn main() {
    let video = "/home/unemotioned/media/nichijou/nichijou_1.mp4";

    let mut vlc = Command::new("mpv")
        .arg("--speed=1.5")
        .arg("--")
        .arg(video)
        .spawn()
        .expect("Failed to launch MPV");

    vlc.wait().expect("Failed to wait for MPV");
}
