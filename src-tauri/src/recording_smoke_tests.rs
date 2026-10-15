//! Explicit opt-in: real file-fed chunk writer -> stop -> local ASR -> local LLM -> SQLite.
//! Never starts the GUI, Recorder, permission prompts, downloads or a real library.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};

fn audio_hashes(directory: &Path) -> BTreeMap<String, String> {
    std::fs::read_dir(directory)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            if !matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("caf" | "wav")
            ) {
                return None;
            }
            Some((
                path.file_name().unwrap().to_string_lossy().to_string(),
                format!("{:x}", Sha256::digest(std::fs::read(path).unwrap())),
            ))
        })
        .collect()
}
fn file_capture(
    lib: &Library,
    id: &str,
    source: &Path,
    helper: &Path,
    mode: &str,
    activity: &activity::Activity,
) -> Capture {
    let meeting = store::load(&lock_db(lib).unwrap(), id).unwrap();
    let offset = recording_offset(&meeting);
    let directory = lib.root.join("recordings").join(id);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("capture.json"),
        json!({"id":id,"source":"synthetic file fixture"}).to_string(),
    )
    .unwrap();
    let previous_audio = meeting["recordings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["id"].as_str().map(str::to_owned))
        .collect::<HashSet<_>>();
    let mut child = Command::new(helper)
        .arg(&directory)
        .arg(source)
        .arg(offset.to_string())
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut ready = String::new();
    stdout.read_line(&mut ready).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&ready).unwrap()["status"],
        "recording",
        "{ready}"
    );
    let error = Arc::new(Mutex::new(None));
    let errors = error.clone();
    let status_thread = std::thread::spawn(move || {
        for line in stdout.lines().map_while(std::result::Result::ok) {
            let message: Value = serde_json::from_str(&line).unwrap();
            if let Some(message) = message["error"].as_str() {
                *errors.lock().unwrap() = Some(message.to_owned());
            }
        }
    });
    Capture {
        child,
        id: id.into(),
        started: Instant::now(),
        offset,
        previous_audio,
        status_thread,
        error,
        _activity: activity.job().unwrap(),
    }
}

#[tokio::test]
#[ignore = "Requires explicit isolated library, file-fed helper and already-installed local models"]
async fn file_audio_completes_transcribes_summarizes_and_reopens_with_real_local_models() {
    let required = |name: &str| {
        std::env::var(name)
            .unwrap_or_else(|_| panic!("Set {name} explicitly; no defaults or downloads"))
    };
    let root = PathBuf::from(required("PATTER_RECORDING_SMOKE_ROOT"));
    assert_eq!(
        std::fs::read_to_string(root.join(".synthetic-test-library")).unwrap(),
        "synthetic-only\n"
    );
    assert!(
        !root.join("patter.sqlite3").exists(),
        "Refusing an existing library"
    );
    let source = PathBuf::from(required("PATTER_RECORDING_SMOKE_AUDIO"));
    let capture_helper = PathBuf::from(required("PATTER_RECORDING_SMOKE_HELPER"));
    let model = required("PATTER_RECORDING_SMOKE_MODEL");
    let endpoint = models::local_url(&required("PATTER_RECORDING_SMOKE_ENDPOINT")).unwrap();
    let lib = store::open(&root).unwrap();
    let activity = activity::Activity::default();
    activity.set_root(&root).unwrap();
    let preferences = json!({"endpoint":endpoint,"model":model,"transcriptionModel":"parakeet-v3","calendarEnabled":false});
    lock_db(&lib)
        .unwrap()
        .execute(
            "INSERT INTO preferences(id,data) VALUES(1,?)",
            [preferences.to_string()],
        )
        .unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let original = store::save_checked(&mut lock_db(&lib).unwrap(),json!({"id":id,"revision":0,"title":"Synthetic recording smoke test","createdAt":chrono::Utc::now().to_rfc3339(),"duration":0,"notes":"Keep original synthetic notes.","summary":"Previous synthetic summary","decisions":[],"actions":[{"id":"original-action","text":"Retained synthetic action","done":true}],"transcript":[],"recordings":[],"archived":false})).unwrap();
    let capture = file_capture(&lib, &id, &source, &capture_helper, "normal", &activity);
    let finalized = finish_capture(&lib, capture).await.unwrap();
    assert!(!finalized["recordings"].as_array().unwrap().is_empty());
    assert_eq!(finalized["recordingProcessing"]["stage"], "transcript");
    assert_eq!(finalized["recordingProcessing"]["status"], "pending");
    assert_eq!(finalized["summary"], original["summary"]);
    let directory = root.join("recordings").join(&id);
    let before = audio_hashes(&directory);
    let parakeet = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/patter-parakeet");
    let saved = recording_processing::run(
        &lib,
        &id,
        |job| {
            let helper = parakeet.clone();
            let activity = activity.clone();
            async move { services::generate(&job, helper, activity).await }
        },
        |stage| println!("Real local processing stage: {stage}"),
    )
    .await
    .unwrap();
    std::fs::write(
        root.join("successful-meeting.json"),
        serde_json::to_vec_pretty(&saved).unwrap(),
    )
    .unwrap();
    assert_eq!(saved["recordingProcessing"]["status"], "complete");
    assert_eq!(saved["recordingProcessing"]["stage"], "complete");
    assert!(!saved["transcript"].as_array().unwrap().is_empty());
    assert!(saved["summary"].as_str().unwrap().starts_with("### "));
    assert_eq!(saved["summarySource"]["summaryFormatVersion"], 1);
    assert_eq!(saved["summaryFreshness"], "current");
    assert_eq!(saved["notes"], original["notes"]);
    assert_eq!(
        audio_hashes(&directory),
        before,
        "Transcription changed captured audio"
    );
    let reopened = store::open(&root).unwrap();
    assert_eq!(
        store::load(&lock_db(&reopened).unwrap(), &id).unwrap(),
        saved
    );
    let historic: String = lock_db(&reopened)
        .unwrap()
        .query_row(
            "SELECT data FROM versions WHERE id=? AND revision=1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&historic).unwrap()["actions"],
        original["actions"]
    );
    recording_processing::run(
        &reopened,
        &id,
        |_| async { panic!("Completed job must not rerun inference") },
        |_| {},
    )
    .await
    .unwrap();
    let generation = saved["recordingProcessing"]["generation"].clone();
    let failure = finish_capture(
        &lib,
        file_capture(&lib, &id, &source, &capture_helper, "failed", &activity),
    )
    .await
    .unwrap();
    assert!(
        failure["recordings"].as_array().unwrap().len()
            > saved["recordings"].as_array().unwrap().len()
    );
    assert_eq!(
        failure["recordingProcessing"]["generation"], generation,
        "Failed capture must not queue new generation"
    );
    assert_eq!(failure["summary"], saved["summary"]);
    assert_eq!(failure["actions"], saved["actions"]);
    let empty = finish_capture(
        &lib,
        file_capture(&lib, &id, &source, &capture_helper, "empty", &activity),
    )
    .await
    .unwrap();
    assert_eq!(empty["recordings"], failure["recordings"]);
    assert_eq!(empty["recordingProcessing"]["generation"], generation);
    assert_eq!(empty["summary"], saved["summary"]);
    assert_eq!(empty["actions"], saved["actions"]);
    assert_eq!(
        store::preferences(&lock_db(&reopened).unwrap()).unwrap(),
        preferences
    );
    std::fs::write(
        root.join("final-meeting.json"),
        serde_json::to_vec_pretty(&empty).unwrap(),
    )
    .unwrap();
    std::fs::write(root.join("checks.json"),serde_json::to_vec_pretty(&json!({"recordingCompletion":true,"realLocalParakeet":true,"realLocalSummary":true,"savedAndReopened":true,"priorVersionRetained":true,"audioUnchangedDuringInference":true,"completedJobNotRepeated":true,"failedCaptureRetainsAudioAndSummary":true,"emptyCaptureDoesNotQueue":true,"deviceCapture":false,"syntheticSourceSha256":format!("{:x}",Sha256::digest(std::fs::read(&source).unwrap()))})).unwrap()).unwrap();
    println!("Recording smoke evidence: {}", root.display());
}
