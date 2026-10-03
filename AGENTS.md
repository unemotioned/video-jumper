# Video Jumper

## Commands

- Validate changes with `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
- The package's binary is named `vj` (not `video-jumper`): run it with `cargo run --bin vj`.
- Videos and optional `.vj.toml` are read from the process working directory. To target another directory, run there with `cargo run --manifest-path /absolute/path/to/vj-ai/Cargo.toml --bin vj`.
- There are currently no wired-up tests; the README's `src/tests.rs` reference is stale. A successful `cargo test` alone does not verify behavior.

## Behavior Constraints

- Preserve discovery of direct directory entries only, case-insensitive `avi`, `mov`, `mp4`, and `ts` extensions, and natural filename sorting.
- Selection numbering starts at 0 only when the first sorted filename's stem ends in an all-zero digit suffix (e.g. `episode00.mp4`); otherwise it starts at 1. Space-separated numbers and inclusive `a:b`, `a:`, `:b` ranges preserve order and duplicates.
- `src/main.rs` loads and validates all `.vj.toml` entries before prompting or launching playback. Config keys match filenames exactly; see README for the schema.
- Playback requires Unix and `mpv` on `PATH`. All selected filenames are passed to one mpv process after `--`, at speed `1.5`; stdout/stderr are discarded.
- Keep the private IPC socket path short: `src/playback.rs` deliberately uses a temporary directory under `/tmp` to avoid macOS Unix socket path-length limits.
- Jump state resets per playlist entry, not filename, so repeated selections replay jumps. Frame positions use `container-fps` to convert to media seconds and wait when FPS is unavailable; `is_opening` is metadata, not a switch.
