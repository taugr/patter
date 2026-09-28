use super::*;
fn fixture() -> (Library, Access, String) {
    let root = std::path::PathBuf::from("/tmp")
        .join(format!("patter-agent-test-{}", uuid::Uuid::new_v4()));
    let lib = store::open(&root).unwrap();
    let access = Access::new(root.join("agent")).unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    store::save(&mut lib.db.lock().unwrap(),json!({"id":id,"title":"Pricing discussion","createdAt":"2026-09-28T10:00:00Z","notes":"Keep original notes","summary":"Agreed pricing","transcript":[{"start":12,"speaker":"Alex","text":"Keep the price fixed"}],"actions":[{"id":"a","text":"Follow up","done":false}],"recordings":[{"id":"audio","path":"/retained.wav"}],"archived":false})).unwrap();
    (lib, access, id)
}
#[test]
fn access_is_opt_in_and_read_only_until_enabled() {
    let (lib, access, id) = fixture();
    assert!(
        library_call(&lib, &access, "search_conversations", json!({}))
            .unwrap_err()
            .contains("off")
    );
    access
        .configure(Config {
            enabled: true,
            ..Default::default()
        })
        .unwrap();
    let result = library_call(
        &lib,
        &access,
        "search_conversations",
        json!({"query":"pricing","from":"2026-09-28"}),
    )
    .unwrap();
    assert_eq!(result["total"], 1);
    assert!(library_call(
        &lib,
        &access,
        "edit_conversation",
        json!({"id":id,"expected_revision":1,"notes":"changed"})
    )
    .is_err());
    assert!(library_call(&lib, &access, "search_conversations", json!({"limit":51})).is_err());
    assert!(library_call(
        &lib,
        &access,
        "get_conversation",
        json!({"id":id,"section":"credentials"})
    )
    .is_err());
    assert!(library_call(
        &lib,
        &access,
        "edit_conversation",
        json!({"id":id,"expected_revision":1,"recordings":[]})
    )
    .is_err());
    access.configure(Config::default()).unwrap();
    assert!(library_call(
        &lib,
        &access,
        "get_conversation",
        json!({"id":id,"section":"notes"})
    )
    .is_err());
}
#[test]
fn edits_preserve_audio_history_and_reject_stale_clients() {
    let (lib, access, id) = fixture();
    access
        .configure(Config {
            enabled: true,
            allow_edit: true,
            allow_processing: false,
        })
        .unwrap();
    library_call(&lib,&access,"edit_conversation",json!({"id":id,"expected_revision":1,"notes":"Agent note","action_id":"a","action_done":true})).unwrap();
    assert!(library_call(
        &lib,
        &access,
        "edit_conversation",
        json!({"id":id,"expected_revision":1,"title":"Stale"})
    )
    .unwrap_err()
    .contains("REVISION_CONFLICT"));
    let db = lib.db.lock().unwrap();
    let saved = store::load(&db, &id).unwrap();
    assert_eq!(saved["recordings"][0]["path"], "/retained.wav");
    assert_eq!(saved["actions"][0]["done"], true);
    assert_eq!(saved["agentChange"]["source"], "local-mcp");
    let raw: String = db
        .query_row(
            "SELECT data FROM versions WHERE id=? AND revision=1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&raw).unwrap()["notes"],
        "Keep original notes"
    );
    assert!(db.execute("DELETE FROM versions", []).is_err());
}
#[test]
fn transcript_pages_and_original_versions_are_readable_without_paths() {
    let (lib, access, id) = fixture();
    access
        .configure(Config {
            enabled: true,
            ..Default::default()
        })
        .unwrap();
    let result = library_call(
        &lib,
        &access,
        "get_conversation",
        json!({"id":id,"section":"transcript","revision":1,"limit":1}),
    )
    .unwrap();
    assert_eq!(result["content"][0]["start"], 12);
    assert_eq!(result["content"][0]["speaker"], "Alex");
    let overview = library_call(
        &lib,
        &access,
        "get_conversation",
        json!({"id":id,"section":"overview"}),
    )
    .unwrap();
    assert!(!overview.to_string().contains("/retained.wav"));
    let notes = library_call(
        &lib,
        &access,
        "get_conversation",
        json!({"id":id,"section":"notes","offset":5,"limit":8}),
    )
    .unwrap();
    assert_eq!(notes["content"], "original");
}
#[test]
fn processing_refuses_to_overwrite_a_changed_conversation() {
    let (lib, _, id) = fixture();
    let job = services::prepare(&lib, &id, "transcript", None).unwrap();
    let mut changed = job.meeting.clone();
    changed["notes"] = json!("Concurrent edit");
    store::save_checked(&mut lib.db.lock().unwrap(), changed).unwrap();
    assert!(
        services::finish(&lib, &job, json!([{"start":0,"text":"Generated"}]), None)
            .unwrap_err()
            .contains("REVISION_CONFLICT")
    );
    assert_eq!(
        store::load(&lib.db.lock().unwrap(), &id).unwrap()["transcript"][0]["start"],
        12
    );
}
#[tokio::test]
async fn official_mcp_client_initializes_lists_calls_and_handles_missing_app() {
    use rmcp::{model::CallToolRequestParams, ServiceExt};
    let (lib, access, id) = fixture();
    access
        .configure(Config {
            enabled: true,
            ..Default::default()
        })
        .unwrap();
    let socket = access.dir.join("test.sock");
    let (_host_lock, bound) = transport::bind_socket(&socket).unwrap();
    let listener = tokio::net::UnixListener::from_std(bound).unwrap();
    let server = tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut line = String::new();
        tokio::io::BufReader::new(&mut stream)
            .read_line(&mut line)
            .await
            .unwrap();
        let req: Value = serde_json::from_str(&line).unwrap();
        let value = library_call(
            &lib,
            &access,
            req["tool"].as_str().unwrap(),
            req["arguments"].clone(),
        )
        .unwrap();
        stream
            .write_all(format!("{}\n", json!({"result":value})).as_bytes())
            .await
            .unwrap();
    });
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let bridge = transport::Bridge { socket };
    let server_mcp = tokio::spawn(async move {
        bridge
            .serve(server_io)
            .await
            .unwrap()
            .waiting()
            .await
            .unwrap();
    });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 9);
    assert!(!tools.iter().any(|t| t.name.contains("delete")));
    let response = client
        .call_tool(
            CallToolRequestParams::new("get_conversation").with_arguments(
                json!({"id":id,"section":"transcript"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_ne!(response.is_error, Some(true));
    assert!(serde_json::to_string(&response).unwrap().contains("Alex"));
    server.await.unwrap();
    let missing = client
        .call_tool(CallToolRequestParams::new("patter_status"))
        .await
        .unwrap();
    assert_eq!(missing.is_error, Some(true));
    client.cancel().await.unwrap();
    server_mcp.await.unwrap();
}

#[test]
fn revoking_and_regranting_processing_does_not_commit_an_old_job() {
    let (lib, access, id) = fixture();
    let allowed = Config {
        enabled: true,
        allow_processing: true,
        allow_edit: false,
    };
    access.configure(allowed.clone()).unwrap();
    let generation = access.control.lock().unwrap().generation;
    let prepared = services::prepare(&lib, &id, "transcript", None).unwrap();
    access.configure(Config::default()).unwrap();
    assert!(finish_processing(&lib, &access, &prepared, json!([]), generation).is_err());
    access.configure(allowed).unwrap();
    assert!(
        finish_processing(&lib, &access, &prepared, json!([]), generation)
            .unwrap_err()
            .contains("permissions changed")
    );
    assert_eq!(
        store::load(&lib.db.lock().unwrap(), &id).unwrap()["revision"],
        1
    );
    let fresh = access.control.lock().unwrap().generation;
    let saved = finish_processing(
        &lib,
        &access,
        &prepared,
        json!([{"start":0,"text":"New transcript"}]),
        fresh,
    )
    .unwrap();
    assert_eq!(saved["revision"], 2);
    let normal = store::save_checked(&mut lib.db.lock().unwrap(), saved).unwrap();
    assert!(normal["agentChange"].is_null());
}
#[test]
fn insecure_socket_directories_and_unknown_edit_fields_are_rejected() {
    use std::os::unix::fs::PermissionsExt;
    let (lib, access, id) = fixture();
    access
        .configure(Config {
            enabled: true,
            allow_edit: true,
            allow_processing: false,
        })
        .unwrap();
    assert!(library_call(
        &lib,
        &access,
        "edit_conversation",
        json!({"id":id,"expected_revision":1,"recordings":[]})
    )
    .is_err());
    std::fs::set_permissions(&access.dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(transport::private_directory(&access.dir).is_err());
}

#[test]
fn host_lock_protects_live_socket_and_restarts_after_exit() {
    use std::os::unix::fs::MetadataExt;
    let (_, access, _) = fixture();
    let socket = access.dir.join("agent.sock");
    let (lock, listener) = transport::bind_socket(&socket).unwrap();
    let inode = std::fs::metadata(&socket).unwrap().ino();
    assert!(transport::bind_socket(&socket).is_err());
    assert_eq!(std::fs::metadata(&socket).unwrap().ino(), inode);
    drop(listener);
    drop(lock);
    assert!(transport::bind_socket(&socket).is_ok());
}
