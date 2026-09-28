//! Explicit, opt-in download + inference check on caller-supplied test audio.
//! cargo run --example transcription-smoke -- /tmp/patter-model-proof parakeet-v3 public/sample-conversation.wav
#[allow(dead_code)]
mod store {
    pub type Result<T> = std::result::Result<T, String>;
}
#[path = "../src/transcription.rs"]
mod transcription;
#[allow(dead_code)]
#[path = "../src/transcription_models.rs"]
mod transcription_models;
use serde_json::json;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "Expected test library directory, catalog model ID and audio file"
    );
    let root = std::path::PathBuf::from(&args[1]);
    let model = transcription_models::model(&args[2]).unwrap();
    println!(
        "Downloading/verifying {} ({} bytes)",
        model.name,
        model.size()
    );
    let state = transcription_models::Downloads::default();
    tauri::async_runtime::block_on(transcription_models::download(&root, &model, &state)).unwrap();
    let directory = root.join("recordings/smoke");
    std::fs::create_dir_all(&directory).unwrap();
    let audio = directory.join("sample.wav");
    std::fs::copy(&args[3], &audio).unwrap();
    let original = std::fs::read(&audio).unwrap();
    let helper = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/patter-parakeet");
    let meeting =
        json!({"id":"smoke","recordings":[{"path":audio,"offset":2.5,"track":"Test audio"}]});
    let transcript = transcription::transcribe(
        &root,
        &meeting,
        &json!({"transcriptionModel":model.id}),
        &helper,
    )
    .unwrap();
    assert!(!transcript.is_empty(), "Expected speech in test audio");
    assert!(transcript
        .iter()
        .all(|s| s["start"].as_f64().unwrap() >= 2.5 && s["speaker"] == "Test audio"));
    assert_eq!(
        std::fs::read(&audio).unwrap(),
        original,
        "Original recording must not change"
    );
    println!("{}", serde_json::to_string_pretty(&transcript).unwrap());
}
