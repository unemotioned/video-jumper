use super::selection_start;

#[test]
fn zero_episode_suffix_starts_selection_at_zero() {
    for name in [
        "Episode 0.avi",
        "Episode 00.MOV",
        "Show_000.ts",
        "nicjijou_0.mp4",
    ] {
        assert_eq!(selection_start(name), 0, "{name}");
    }
}

#[test]
fn other_names_start_selection_at_one() {
    for name in [
        "Episode 1.mp4",
        "Episode 10.mp4",
        "Episode 100.mp4",
        "Episode 01.mp4",
        "Episode.mp4",
        "0 introduction.mp4",
    ] {
        assert_eq!(selection_start(name), 1, "{name}");
    }
}
