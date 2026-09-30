use super::{get_property, parse_selection, selection_start};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

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
