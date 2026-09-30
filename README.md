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
provides playback position through mpv's JSON IPC interface, polled about
every 50 milliseconds:

```text
Time:        12.375s | Frame (estimated):          297
```

Time is the current position in the video, so it follows pauses and seeks.
The frame number is mpv's `estimated-frame-number`, which may be approximate,
especially for variable-frame-rate video. Unavailable values appear as `--`.
The socket directory is removed when playback finishes.

## Automatic jumps

Create `.vj.toml` in the directory containing your videos:

```toml
[videos."nichijou_1.mp4"]
jumps = [
  { from = { frame = "3391" }, to = { frame = "5541" }, is_opening = true },
  { from = { frame = "15675" }, to = { frame = "15969" }, is_opening = false },
  { end = { time = "1405.989" } },
]
```

The table key must match the video filename exactly. Add a table for each video
you want to configure. Videos without a configuration play normally.

- `from` / `to`: when playback enters this interval, seek to `to`.
- `end`: once playback reaches this position, finish the current video and move
  to the next selected video. If it is last, playback ends.
- Positions use either `time` in seconds or `frame` (zero-based). Both quoted
  numbers and TOML numbers are accepted. Frame numbers must be whole numbers.
- `is_opening` is optional metadata; both `true` and `false` jumps are applied.

Each jump runs once per playlist entry, including repeated selections of the same
video. Frame positions are converted to seconds using mpv's `container-fps` and
are approximate for variable-frame-rate video. Frame jumps wait if the frame rate
is unavailable. Seeks request exact positioning, but trigger detection is polled
and can be slightly late. Time positions are media time, independent of playback
speed. Invalid configuration is reported before playback starts.

## Source layout

- `src/main.rs` — application flow, error reporting, and exit status.
- `src/config.rs` — `.vj.toml` parsing, validation, and jump rules.
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
