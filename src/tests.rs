use super::playback::get_property;
use super::selection::{parse_selection, selection_start};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use super::config::{Action, Config};
use super::playback::execute_action;

#[test]
fn config_accepts_example_and_triggers_jumps_at_boundaries() {
    let config = Config::parse(
        r#"
[videos."nichijou_1.mp4"]
jumps = [
  { from = { frame = "3391" }, to = { frame = "5541" }, is_opening = true },
  { from = { frame = "15675" }, to = { frame = "15969" }, is_opening = false },
  { end = { time = "1405.989" } },
]
"#,
    )
    .unwrap();
    let jumps = &config.videos["nichijou_1.mp4"].jumps;
    let fps = Some(24.0);
    assert_eq!(jumps[0].action(3390.0 / 24.0, fps), None);
    assert_eq!(
        jumps[0].action(3391.0 / 24.0, fps),
        Some(Action::Seek(5541.0 / 24.0))
    );
    assert_eq!(
        jumps[0].action(5500.0 / 24.0, fps),
        Some(Action::Seek(5541.0 / 24.0))
    );
    assert_eq!(jumps[0].action(5541.0 / 24.0, fps), None);
    assert_eq!(
        jumps[1].action(15675.0 / 24.0, fps),
        Some(Action::Seek(15969.0 / 24.0))
    );
    assert_eq!(jumps[2].action(1405.988, None), None);
    assert_eq!(jumps[2].action(1405.989, None), Some(Action::End));
    assert_eq!(jumps[2].action(1500.0, None), Some(Action::End));
}

#[test]
fn time_jumps_and_frame_end_accept_numeric_values() {
    let config = Config::parse(
        r#"[videos."test.mp4"]
jumps = [{ from = { time = 1.5 }, to = { time = 3 } }, { end = { frame = 120 } }]"#,
    )
    .unwrap();
    let jumps = &config.videos["test.mp4"].jumps;
    assert_eq!(jumps[0].action(1.5, None), Some(Action::Seek(3.0)));
    assert_eq!(jumps[1].action(5.0, Some(24.0)), Some(Action::End));
    for fps in [None, Some(0.0), Some(-1.0), Some(f64::NAN)] {
        assert_eq!(jumps[1].action(5.0, fps), None);
    }
}

#[test]
fn invalid_jump_configuration_is_rejected() {
    for jump in [
        "{ end = { time = -1 } }",
        "{ end = { time = 'NaN' } }",
        "{ end = { time = 'oops' } }",
        "{ end = { frame = 1.5 } }",
        "{ end = { frame = 1, time = 2 } }",
        "{ from = { time = 2 }, to = { time = 1 } }",
        "{ from = { frame = 2 }, to = { frame = 2 } }",
        "{ from = { time = 2 } }",
        "{ end = { time = 2 }, to = { time = 3 } }",
    ] {
        assert!(
            Config::parse(&format!("[videos.'test.mp4']\njumps = [{jump}]")).is_err(),
            "{jump}"
        );
    }
}

#[test]
fn absent_config_is_optional() {
    let directory = tempfile::tempdir().unwrap();
    assert!(Config::load(directory.path()).unwrap().videos.is_empty());
}

#[test]
fn ipc_sends_exact_seek_and_forced_playlist_advance() {
    let (client, server) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let player = std::thread::spawn(move || {
        let mut server = BufReader::new(server);
        for expected in [
            serde_json::json!(["seek", 230.875, "absolute+exact"]),
            serde_json::json!(["playlist-next", "force"]),
            serde_json::json!(["seek", 1.0, "absolute+exact"]),
        ] {
            let mut line = String::new();
            server.read_line(&mut line).unwrap();
            let request: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(request["command"], expected);
            let error = if expected[1] == 1.0 {
                "command failed"
            } else {
                "success"
            };
            writeln!(
                server.get_mut(),
                "{}",
                serde_json::json!({"request_id": 1, "error": error})
            )
            .unwrap();
        }
    });
    let mut client = BufReader::new(client);
    execute_action(&mut client, Action::Seek(230.875)).unwrap();
    execute_action(&mut client, Action::End).unwrap();
    assert!(execute_action(&mut client, Action::Seek(1.0)).is_err());
    player.join().unwrap();
}

#[test]
fn ipc_reads_positions_despite_events_and_unavailable_properties() {
    let (client, server) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let player = std::thread::spawn(move || {
        let mut server = BufReader::new(server);
        for (name, reply) in [
            (
                "time-pos",
                r#"{"request_id":1,"error":"success","data":12.375}"#,
            ),
            (
                "estimated-frame-number",
                r#"{"request_id":1,"error":"success","data":297}"#,
            ),
            (
                "time-pos",
                r#"{"request_id":1,"error":"property unavailable"}"#,
            ),
        ] {
            let mut request = String::new();
            server.read_line(&mut request).unwrap();
            let request: serde_json::Value = serde_json::from_str(&request).unwrap();
            assert_eq!(
                request["command"],
                serde_json::json!(["get_property", name])
            );
            assert_eq!(request["request_id"], 1);
            writeln!(
                server.get_mut(),
                "{{\"event\":\"playback-restart\"}}\n{reply}"
            )
            .unwrap();
        }
    });
    let mut client = BufReader::new(client);
    assert_eq!(get_property(&mut client, "time-pos").unwrap(), 12.375);
    assert_eq!(
        get_property(&mut client, "estimated-frame-number").unwrap(),
        297
    );
    assert!(get_property(&mut client, "time-pos").unwrap().is_null());
    player.join().unwrap();
    assert!(get_property(&mut client, "time-pos").is_err());
}

#[test]
fn open_ranges_include_the_selected_endpoint() {
    assert_eq!(parse_selection("2:", 1, 5), Some(vec![1, 2, 3, 4]));
    assert_eq!(parse_selection(":4", 1, 5), Some(vec![0, 1, 2, 3]));
    assert_eq!(parse_selection("5:", 1, 5), Some(vec![4]));
    assert_eq!(parse_selection(":1", 1, 5), Some(vec![0]));
}

#[test]
fn ranges_use_zero_based_display_numbers_when_applicable() {
    assert_eq!(parse_selection(":2", 0, 4), Some(vec![0, 1, 2]));
    assert_eq!(parse_selection("2:", 0, 4), Some(vec![2, 3, 4]));
    assert_eq!(parse_selection(":", 0, 0), Some(vec![0]));
}

#[test]
fn ranges_and_numbers_preserve_queue_order() {
    assert_eq!(
        parse_selection("4: 1 2:3 :2", 1, 5),
        Some(vec![3, 4, 0, 1, 2, 0, 1])
    );
    assert_eq!(parse_selection(":", 1, 3), Some(vec![0, 1, 2]));
}

#[test]
fn invalid_ranges_reject_the_entire_queue() {
    for input in ["1 0:", ":4", "4:", ":0", "3:2", "1:2:3", "a:", ":a"] {
        assert_eq!(parse_selection(input, 1, 3), None, "{input:?}");
    }
}

#[test]
fn queue_preserves_input_order_and_repeats() {
    assert_eq!(parse_selection("3 1 2 3", 1, 3), Some(vec![2, 0, 1, 2]));
}

#[test]
fn queue_supports_zero_based_selection_and_whitespace() {
    assert_eq!(parse_selection(" 2\t0 1\n", 0, 2), Some(vec![2, 0, 1]));
}

#[test]
fn single_video_selection_still_works() {
    assert_eq!(parse_selection("1", 1, 3), Some(vec![0]));
    assert_eq!(parse_selection("0", 0, 0), Some(vec![0]));
}

#[test]
fn invalid_selection_rejects_the_entire_queue() {
    for input in ["", " \n", "1 nope 2", "1 4", "0 1", "-1", "1.5"] {
        assert_eq!(parse_selection(input, 1, 3), None, "{input:?}");
    }
}

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
