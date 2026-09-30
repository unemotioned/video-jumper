# Video Jumper

## Commands

- Validate changes with `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
- The package's binary is named `vj` (not `video-jumper`): run it with `cargo run --bin vj`.
- `vj` lists videos from its process working directory. To run it against another directory without copying the manifest, run from that directory: `cargo run --manifest-path /path/to/video-jumper/Cargo.toml --bin vj`.

## Behavior Constraints

- The only application entrypoint is `src/main.rs`; this is a single-binary Cargo package using Rust edition 2024.
- Preserve the current selection contract: direct entries in the working directory only; case-insensitive `avi`, `mov`, `mp4`, and `ts` extensions; natural filename sorting; and 1-based user selection.
- Playback requires `mpv` on `PATH`. The selected filename is passed to `mpv` after `--`, currently with playback speed `1.5`.
