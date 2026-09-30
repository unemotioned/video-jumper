# Video Jumper

Media player controller CLI written in [Rust](https://rust-lang.org/).

After watching the anime [Nichijou](https://nichijou.fandom.com/wiki/Nichijou)
more than 10 times, I came up with an idea:

> What if I could skip specific parts of a video without editing the original file?

---

## Playback status

Requires `mpv` on `PATH` and macOS or Linux. Run `vj` in the video directory,
then select videos to play at 1.5× speed.

mpv's stdout and stderr are discarded (`/dev/null`). A private Unix socket
provides playback position through mpv's JSON IPC interface, refreshed about
four times per second:

```text
Time:        12.375s | Frame (estimated):          297
```

Time is the current position in the video, so it follows pauses and seeks.
The frame number is mpv's `estimated-frame-number`, which may be approximate,
especially for variable-frame-rate video. Unavailable values appear as `--`.
The socket directory is removed when playback finishes.

## Source layout

- `src/main.rs` — application flow, error reporting, and exit status.
- `src/videos.rs` — video discovery and natural filename sorting.
- `src/selection.rs` — numbered listing, input prompt, and selection parsing.
- `src/playback.rs` — mpv process lifecycle and live status over JSON IPC.
- `src/tests.rs` — selection and IPC regression tests.

Validate changes with:

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## TODO

- [x] spawn mpv and play video
- [x] start with vj command
- [x] list videos in current directory
- [x] play selected videos
- [x] get current videos time/frame live
- [ ] get playback speed info by passing option and argument
