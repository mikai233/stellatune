use std::time::{Duration, Instant};

use stellatune_core::{Command, ControlCommand, ControlScope, Event, HostEventTopic};

use super::bus::{
    build_control_finished_event_json, build_control_result_event_json,
    drain_finished_by_player_event, drain_timed_out_pending,
};
use super::control::{control_wait_kind, parse_player_control};
use super::types::{ControlWaitKind, PendingControlFinish};

#[test]
fn parse_player_control_supports_extended_commands() {
    let load_track_ref = serde_json::json!({
        "command": "load_track_ref",
        "track": {
            "source_id": "local",
            "track_id": "a.flac",
            "locator": "C:/music/a.flac"
        }
    });
    let cmd = parse_player_control(&load_track_ref)
        .expect("parse")
        .expect("command");
    assert!(matches!(cmd, Command::LoadTrackRef { .. }));

    let set_output_device = serde_json::json!({
        "command": "set_output_device",
        "backend": "shared",
        "device_id": "default",
    });
    let cmd = parse_player_control(&set_output_device)
        .expect("parse")
        .expect("command");
    assert!(matches!(
        cmd,
        Command::SetOutputDevice {
            backend: stellatune_core::AudioBackend::Shared,
            ..
        }
    ));

    let preload_track = serde_json::json!({
        "command": "preload_track",
        "path": "C:/music/a.flac"
    });
    let cmd = parse_player_control(&preload_track)
        .expect("parse")
        .expect("command");
    assert!(matches!(cmd, Command::PreloadTrack { position_ms: 0, .. }));
}

#[test]
fn control_result_echoes_request_id() {
    let root = serde_json::json!({
        "scope": "player",
        "command": "play",
        "request_id": "req-1"
    });

    let ok = build_control_result_event_json(Some(&root), None);
    let ok_v: serde_json::Value = serde_json::from_str(&ok).expect("json");
    assert_eq!(
        ok_v["topic"],
        serde_json::json!(HostEventTopic::HostControlResult.as_str())
    );
    assert_eq!(ok_v["request_id"], serde_json::json!("req-1"));
    assert_eq!(ok_v["ok"], serde_json::json!(true));

    let err = build_control_result_event_json(Some(&root), Some("failed"));
    let err_v: serde_json::Value = serde_json::from_str(&err).expect("json");
    assert_eq!(err_v["request_id"], serde_json::json!("req-1"));
    assert_eq!(err_v["ok"], serde_json::json!(false));
    assert_eq!(err_v["error"], serde_json::json!("failed"));
}

#[test]
fn control_wait_kind_maps_common_commands() {
    let play = serde_json::json!({
        "scope": "player",
        "command": "play"
    });
    assert_eq!(
        control_wait_kind(&play),
        ControlWaitKind::PlayerState(stellatune_core::PlayerState::Playing)
    );

    let search = serde_json::json!({
        "scope": "library",
        "command": "search"
    });
    assert_eq!(
        control_wait_kind(&search),
        ControlWaitKind::LibrarySearchResult
    );
}

#[test]
fn control_finished_event_json_contains_error() {
    let raw = build_control_finished_event_json(
        Some(serde_json::json!("req-9")),
        ControlScope::Library,
        Some(ControlCommand::ScanAll),
        Some("control finish timeout"),
    );
    let v: serde_json::Value = serde_json::from_str(&raw).expect("json");
    assert_eq!(
        v["topic"],
        serde_json::json!(HostEventTopic::HostControlFinished.as_str())
    );
    assert_eq!(v["request_id"], serde_json::json!("req-9"));
    assert_eq!(v["ok"], serde_json::json!(false));
    assert_eq!(v["error"], serde_json::json!("control finish timeout"));
}

#[test]
fn player_event_finishes_pending_control() {
    let mut pending = vec![PendingControlFinish {
        plugin_id: "p.demo".to_string(),
        request_id: Some(serde_json::json!("req-play")),
        scope: ControlScope::Player,
        command: Some(ControlCommand::Play),
        wait: ControlWaitKind::PlayerState(stellatune_core::PlayerState::Playing),
        deadline: Instant::now() + Duration::from_secs(1),
    }];

    let done = drain_finished_by_player_event(
        &mut pending,
        &Event::StateChanged {
            state: stellatune_core::PlayerState::Playing,
        },
    );

    assert_eq!(done.len(), 1);
    assert!(pending.is_empty());
    assert_eq!(done[0].command, Some(ControlCommand::Play));
}

#[test]
fn timeout_drains_pending_control() {
    let mut pending = vec![PendingControlFinish {
        plugin_id: "p.demo".to_string(),
        request_id: Some(serde_json::json!("req-timeout")),
        scope: ControlScope::Library,
        command: Some(ControlCommand::ScanAll),
        wait: ControlWaitKind::LibraryScanFinished,
        deadline: Instant::now() - Duration::from_millis(1),
    }];

    let timed_out = drain_timed_out_pending(&mut pending, Instant::now());
    assert_eq!(timed_out.len(), 1);
    assert!(pending.is_empty());
    assert_eq!(timed_out[0].command, Some(ControlCommand::ScanAll));
}
