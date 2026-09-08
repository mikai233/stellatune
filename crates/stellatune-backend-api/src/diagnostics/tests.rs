use super::*;

#[test]
fn diagnostics_replays_pages_full_details_and_previous_sessions() {
    let root = tempfile::tempdir().unwrap();
    let service = start();
    let mut receiver = service.subscribe();
    let id = service.record(LogRecord::new(
        "rust",
        "ERROR",
        "output",
        "device failed".into(),
        "root cause\nframe 1\nframe 2".into(),
    ));
    let session = service.configure(root.path().into()).unwrap();
    for i in 0..4 {
        service.record(LogRecord::new(
            "plugin",
            "INFO",
            "test",
            format!("entry {i}"),
            String::new(),
        ));
    }
    service.flush();
    assert_eq!(receiver.try_recv().unwrap().id, id);
    let page = service.query(&session, 0, 2, "", "", "").unwrap();
    assert_eq!(page.records.len(), 2);
    assert_eq!(page.next_offset, Some(2));
    assert!(page.records[0].details.is_empty());
    assert_eq!(
        service.detail(&id).unwrap().details,
        "root cause\nframe 1\nframe 2"
    );
    assert_eq!(
        service
            .query(&session, 0, 20, "ERROR", "rust", "frame 2")
            .unwrap()
            .records
            .len(),
        1
    );
    let restarted = start();
    restarted.configure(root.path().into()).unwrap();
    assert!(restarted.sessions().unwrap().contains(&session));
    assert_eq!(restarted.detail(&id).unwrap().message, "device failed");
    let export = root.path().join("export.txt");
    restarted.export(&session, export.clone()).unwrap();
    assert!(
        std::fs::read_to_string(export)
            .unwrap()
            .contains("frame 1\nframe 2")
    );
}

#[test]
fn bounded_queue_reports_loss_and_redacts_secrets() {
    let service = start();
    service.record(LogRecord::new(
        "flutter",
        "ERROR",
        "test",
        "Authorization: secret".into(),
        "Cookie: session=secret\nnext line".into(),
    ));
    service.record(LogRecord::new(
        "plugin",
        "INFO",
        "test",
        "x".repeat(CACHE_BYTES + 1),
        String::new(),
    ));
    service.flush();
    let records = service.cache.lock().unwrap();
    assert!(records.iter().any(|r| r.message.contains("Dropped 1")));
    assert!(
        records
            .iter()
            .all(|r| !r.message.contains("secret") && !r.details.contains("secret"))
    );
    assert!(records.iter().any(|r| r.details.contains("next line")));
}

#[test]
fn files_rotate_cache_evicts_and_lagging_subscribers_resynchronize() {
    let root = tempfile::tempdir().unwrap();
    let service = start();
    let mut receiver = service.subscribe();
    let session = service.configure(root.path().into()).unwrap();
    let id = service.record(LogRecord::new(
        "rust",
        "ERROR",
        "test",
        "early error".into(),
        "full detail".into(),
    ));
    service.flush();
    for i in 0..1300 {
        service.record(LogRecord::new(
            "plugin",
            "INFO",
            "test",
            format!("entry {i}"),
            "x".repeat(10000),
        ));
        if i % 40 == 0 {
            service.flush();
        }
    }
    service.flush();
    assert!(matches!(
        receiver.try_recv(),
        Err(broadcast::error::TryRecvError::Lagged(_))
    ));
    assert!(
        service
            .cache
            .lock()
            .unwrap()
            .iter()
            .map(LogRecord::bytes)
            .sum::<usize>()
            <= CACHE_BYTES
    );
    assert!(files::paths(root.path(), Some(&session)).unwrap().len() >= 2);
    assert_eq!(service.detail(&id).unwrap().details, "full detail");
}

#[test]
fn unavailable_storage_keeps_diagnostics_in_memory() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("not-a-directory");
    std::fs::write(&file, "occupied").unwrap();
    let service = start();
    assert!(service.configure(file).is_err());
    let id = service.record(LogRecord::new(
        "flutter",
        "ERROR",
        "startup",
        "load failed".into(),
        "Dart stack".into(),
    ));
    service.flush();
    assert_eq!(service.detail(&id).unwrap().details, "Dart stack");
    assert_eq!(
        service
            .query(service.session_id(), 0, 20, "ERROR", "", "load")
            .unwrap()
            .records
            .len(),
        1
    );
}

#[test]
fn flutter_references_remain_resolvable_after_eviction_and_restart() {
    let root = tempfile::tempdir().unwrap();
    let service = start();
    service.configure(root.path().into()).unwrap();
    let mut record = LogRecord::new(
        "flutter",
        "ERROR",
        "startup",
        "startup failed".into(),
        "complete Dart stack".into(),
    );
    record.id = "flutter:unique-session:1".into();
    let id = service.record(record);
    service.flush();
    let restarted = start();
    restarted.configure(root.path().into()).unwrap();
    assert_eq!(
        restarted.detail(&id).unwrap().details,
        "complete Dart stack"
    );
}

#[test]
fn rotation_write_failure_reports_once_and_keeps_collecting() {
    let root = tempfile::tempdir().unwrap();
    let service = start();
    let session = service.configure(root.path().into()).unwrap();
    std::fs::create_dir(root.path().join(format!("{session}-00001.jsonl"))).unwrap();
    for _ in 0..3 {
        service.record(LogRecord::new(
            "rust",
            "INFO",
            "test",
            "rotation".into(),
            "x".repeat(4 * 1024 * 1024),
        ));
        service.flush();
    }
    let id = service.record(LogRecord::new(
        "rust",
        "ERROR",
        "test",
        "still alive".into(),
        "full detail".into(),
    ));
    service.flush();
    assert_eq!(service.detail(&id).unwrap().details, "full detail");
    assert_eq!(
        service
            .snapshot()
            .iter()
            .filter(|r| r.id.ends_with(":storage"))
            .count(),
        1
    );
}
