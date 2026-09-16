use std::process::Command;

fn main() {
    let video = "/Users/unemotioned/Media/nichijou/nichijou_1.mp4";

    let mut vlc = Command::new("vlc")
        .arg(video)
        .spawn()
        .expect("Failed to launch VLC");

    vlc.wait().expect("Failed to wait for VLC");
}
