//! Synthetic loopback regressions for final, map and reduce termination.
use super::*;
use crate::{recording_processing, store};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

struct Server {
    endpoint: String,
    done: Arc<AtomicBool>,
    worker: std::thread::JoinHandle<Vec<Value>>,
}
impl Server {
    fn start(responses: Vec<Value>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
        let done = Arc::new(AtomicBool::new(false));
        let stopped = done.clone();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut requests = Vec::new();
            while !stopped.load(Ordering::SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "Model fixture timed out");
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert!(line.starts_with("POST /v1/chat/completions "));
                let mut length = 0;
                loop {
                    line.clear();
                    assert!(reader.read_line(&mut line).unwrap() > 0);
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse::<usize>().unwrap();
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                requests.push(serde_json::from_slice(&body).unwrap());
                // Respond to unexpected later calls too, so the request-count
                // assertion fails promptly rather than waiting on a socket.
                let response = responses
                    .get(requests.len() - 1)
                    .cloned()
                    .unwrap_or_else(|| completion(final_text(), Some(json!("stop"))));
                let response = response.to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
            }
            requests
        });
        Self {
            endpoint,
            done,
            worker,
        }
    }
    fn finish(self) -> Vec<Value> {
        self.done.store(true, Ordering::SeqCst);
        self.worker.join().unwrap()
    }
}
fn final_text() -> &'static str {
    r#"{"summary":"Synthetic new summary","decisions":[],"actions":[]}"#
}
fn completion(text: &str, reason: Option<Value>) -> Value {
    let mut value = json!({"choices":[{"message":{"content":text}}]});
    if let Some(reason) = reason {
        value["choices"][0]["finish_reason"] = reason;
    }
    value
}
fn fixture(endpoint: &str, long: bool) -> (store::Library, String, Value) {
    let root =
        std::env::temp_dir().join(format!("patter-completion-test-{}", uuid::Uuid::new_v4()));
    let lib = store::open(&root).unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let mut db = lib.db.lock().unwrap();
    db.execute(
        "INSERT INTO preferences(id,data) VALUES(1,?)",
        [json!({"endpoint":endpoint,"model":"synthetic-fixture"}).to_string()],
    )
    .unwrap();
    let previous = store::save_checked(&mut db, json!({
        "id":id,"revision":0,"title":"Synthetic completion fixture","notes":"Synthetic notes only.",
        "transcript":[{"start":0,"text":if long {"Synthetic discussion. ".repeat(600)} else {"Synthetic discussion with enough context for a summary.".into()}}],
        "recordings":[],"summary":"Previous summary","decisions":["Previous decision"],
        "actions":[{"id":"previous-action","text":"Previous action","done":true}],
        "recordingProcessing":{"generation":"synthetic","stage":"summary","status":"pending","audioIds":[]}
    })).unwrap();
    drop(db);
    (lib, id, previous)
}
async fn run(lib: &store::Library, id: &str) -> Result<Value> {
    recording_processing::run(
        lib,
        id,
        |job| async move {
            assert_eq!(job.kind, "summary");
            summarize(&job.meeting, &job.preferences).await
        },
        |_| {},
    )
    .await
}
fn preserves_previous(lib: &store::Library, id: &str, previous: &Value) {
    let db = lib.db.lock().unwrap();
    let saved = store::load(&db, id).unwrap();
    for field in ["summary", "decisions", "actions", "notes", "transcript"] {
        assert_eq!(saved[field], previous[field], "Changed {field}");
    }
    assert_eq!(saved["recordingProcessing"]["status"], "failed");
    assert_eq!(saved["recordingProcessing"]["stage"], "summary");
    assert!(saved["recordingProcessing"]["error"].is_string());
    let raw: String = db
        .query_row(
            "SELECT data FROM versions WHERE id=? AND revision=1",
            [id],
            |r| r.get(0),
        )
        .unwrap();
    let retained: Value = serde_json::from_str(&raw).unwrap();
    for field in ["summary", "decisions", "actions"] {
        assert_eq!(retained[field], previous[field]);
    }
    drop(db);
    let reopened = store::open(&lib.root).unwrap();
    assert_eq!(
        store::load(&reopened.db.lock().unwrap(), id).unwrap(),
        saved
    );
}

#[tokio::test]
async fn explicit_incomplete_final_responses_keep_previous_content_and_history() {
    for reason in ["length", "content_filter"] {
        // Even fully parseable final JSON must not certify an explicitly
        // incomplete generation, including a long transcript's final request.
        for long in [false, true] {
            let mut responses = if long {
                vec![completion("Synthetic portion.", Some(json!("stop"))); 2]
            } else {
                vec![]
            };
            responses.push(completion(final_text(), Some(json!(reason))));
            let server = Server::start(responses);
            let (lib, id, previous) = fixture(&server.endpoint, long);
            let error = run(&lib, &id).await.unwrap_err();
            assert!(error.contains("Your previous summary is kept"));
            assert_eq!(server.finish().len(), if long { 3 } else { 1 });
            preserves_previous(&lib, &id, &previous);
            std::fs::remove_dir_all(&lib.root).unwrap();
        }
    }
}

#[tokio::test]
async fn incomplete_map_and_reduce_responses_stop_before_any_later_request() {
    for reason in ["length", "content_filter"] {
        for target in 0..4 {
            // Targets 0/1 are map calls; targets 2/3 are reduce calls after
            // map outputs jointly cross the 14,000-character threshold.
            let large = "Synthetic intermediate. ".repeat(350);
            let mut responses = vec![
                completion(&large, Some(json!("stop"))),
                completion(&large, Some(json!("stop"))),
                completion("Synthetic condensed portion.", Some(json!("stop"))),
                completion("Synthetic condensed portion.", Some(json!("stop"))),
                completion(final_text(), Some(json!("stop"))),
            ];
            responses[target] = completion(
                "Synthetic partial action was assigned to",
                Some(json!(reason)),
            );
            let server = Server::start(responses);
            let (lib, id, previous) = fixture(&server.endpoint, true);
            assert!(run(&lib, &id).await.is_err());
            let requests = server.finish();
            assert_eq!(requests.len(), target + 1);
            assert!(!requests.last().unwrap()["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("Return only a JSON object"));
            if target >= 2 {
                assert!(requests[target]["messages"][0]["content"]
                    .as_str()
                    .unwrap()
                    .contains("Condense these meeting summaries"));
            }
            preserves_previous(&lib, &id, &previous);
            std::fs::remove_dir_all(&lib.root).unwrap();
        }
    }
}

#[tokio::test]
async fn stop_missing_null_and_unknown_reasons_remain_compatible_at_every_stage() {
    for reason in [
        Some(json!("stop")),
        None,
        Some(Value::Null),
        Some(json!("vendor_done")),
    ] {
        for long in [false, true] {
            let large = "Synthetic intermediate. ".repeat(350);
            let responses = if long {
                vec![
                    completion(&large, reason.clone()),
                    completion(&large, reason.clone()),
                    completion("Synthetic condensed portion.", reason.clone()),
                    completion("Synthetic condensed portion.", reason.clone()),
                    completion(final_text(), reason.clone()),
                ]
            } else {
                vec![completion(final_text(), reason.clone())]
            };
            let server = Server::start(responses);
            let (lib, id, previous) = fixture(&server.endpoint, long);
            let saved = run(&lib, &id).await.unwrap();
            assert_eq!(server.finish().len(), if long { 5 } else { 1 });
            assert_eq!(saved["summary"], "Synthetic new summary");
            assert_eq!(saved["recordingProcessing"]["status"], "complete");
            assert!(saved["recordingProcessing"]["error"].is_null());
            let raw: String = lib
                .db
                .lock()
                .unwrap()
                .query_row(
                    "SELECT data FROM versions WHERE id=? AND revision=1",
                    [&id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&raw).unwrap()["summary"],
                previous["summary"]
            );
            std::fs::remove_dir_all(&lib.root).unwrap();
        }
    }
}
