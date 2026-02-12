use std::thread;

use super::PluginEventBus;
use crate::runtime::backend_control::BackendControlResponse;

#[test]
fn plugin_acquire_release_cleans_plugin_queue_state() {
    let bus = PluginEventBus::new(16);
    bus.acquire_plugin("dev.test.plugin");
    bus.push_host_event("dev.test.plugin", "{\"t\":1}".to_string());
    assert!(bus.poll_host_event("dev.test.plugin").is_some());

    bus.release_plugin("dev.test.plugin");
    assert!(bus.registered_plugin_ids().is_empty());
    assert!(bus.poll_host_event("dev.test.plugin").is_none());
}

#[test]
fn plugin_refcount_keeps_queue_until_last_lease_drops() {
    let bus = PluginEventBus::new(16);
    bus.acquire_plugin("dev.test.plugin");
    bus.acquire_plugin("dev.test.plugin");
    assert_eq!(
        bus.registered_plugin_ids(),
        vec!["dev.test.plugin".to_string()]
    );

    bus.release_plugin("dev.test.plugin");
    bus.push_host_event("dev.test.plugin", "{\"t\":2}".to_string());
    assert!(bus.poll_host_event("dev.test.plugin").is_some());

    bus.release_plugin("dev.test.plugin");
    assert!(bus.registered_plugin_ids().is_empty());
    bus.push_host_event("dev.test.plugin", "{\"t\":3}".to_string());
    assert!(bus.poll_host_event("dev.test.plugin").is_none());
}

#[test]
fn control_request_roundtrip() {
    let bus = PluginEventBus::new(16);
    let rx = bus.subscribe_control_requests();
    let worker = thread::spawn(move || {
        let request = rx.recv().expect("must receive request");
        assert_eq!(request.plugin_id, "dev.test.plugin");
        let _ = request
            .response_tx
            .send(BackendControlResponse::ok(r#"{\"ok\":true}"#));
    });

    let response = bus
        .send_control_request("dev.test.plugin", r#"{\"command\":\"ping\"}"#.to_string())
        .expect("must dispatch control request");
    assert_eq!(response.status_code, 0);
    assert_eq!(response.response_json, r#"{\"ok\":true}"#);
    worker.join().expect("worker thread must exit cleanly");
}

#[test]
fn control_request_without_handler_returns_no_handler() {
    let bus = PluginEventBus::new(16);
    let response = bus.send_control_request("dev.test.plugin", "{}".to_string());
    assert!(matches!(
        response,
        Err(super::ControlDispatchError::NoHandler)
    ));
}
