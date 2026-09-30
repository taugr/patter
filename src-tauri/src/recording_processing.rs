//! Post-capture sequence adapted to Patter's local services after reviewing
//! Anarlog 916e6db, stt/capture-lifecycle.ts and utils/summary-eligibility.ts.
use crate::{
    services,
    store::{self, Library, Result},
};
use serde_json::{json, Value};
use std::{collections::HashSet, future::Future, sync::Arc};
use tauri::{Emitter, Manager};

#[derive(Default)]
pub struct Jobs(pub Arc<tokio::sync::Mutex<()>>);
fn audio_ids(meeting: &Value) -> HashSet<String> {
    meeting["recordings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["id"].as_str().map(str::to_owned))
        .collect()
}
pub fn queue(
    lib: &Library,
    mut meeting: Value,
    previous: &HashSet<String>,
    successful: bool,
) -> Result<Value> {
    let mut new_ids: Vec<String> = audio_ids(&meeting).difference(previous).cloned().collect();
    if !successful || new_ids.is_empty() {
        return Ok(meeting);
    }
    // A failed earlier transcription still owns unprocessed audio. Carry it
    // into the next generation; completed transcript stages need no replay.
    if meeting["recordingProcessing"]["stage"] == "transcript" {
        if let Some(pending) = meeting["recordingProcessing"]["audioIds"].as_array() {
            new_ids.extend(
                pending
                    .iter()
                    .filter_map(|id| id.as_str().map(str::to_owned)),
            );
            new_ids.sort();
            new_ids.dedup();
        }
    }
    meeting["recordingProcessing"] = json!({"generation":uuid::Uuid::new_v4().to_string(),"audioIds":new_ids,"stage":"transcript","status":"pending"});
    store::save_checked_attributed(&mut *crate::lock_db(lib)?, meeting, None)
}
fn eligibility(transcript: &Value) -> Option<&'static str> {
    let text = transcript
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        Some("No speech was transcribed. Audio is saved.")
    } else if normalized.split_whitespace().count() < 5 || normalized.chars().count() < 160 {
        Some("Transcript too short for an automatic summary. Audio and transcript are saved.")
    } else {
        None
    }
}
fn mark(
    lib: &Library,
    id: &str,
    generation: &str,
    status: &str,
    error: Option<&str>,
) -> Result<Value> {
    let mut db = crate::lock_db(lib)?;
    let mut meeting = store::load(&db, id)?;
    if meeting["recordingProcessing"]["generation"].as_str() != Some(generation) {
        return Err(
            "Recording processing changed. Open the latest conversation before retrying.".into(),
        );
    }
    meeting["recordingProcessing"]["status"] = json!(status);
    meeting["recordingProcessing"]["error"] = json!(error);
    store::save_checked_attributed(&mut db, meeting, None)
}
// Injected generation lets tests exercise the full durable sequence without
// device capture, real models, or transmitting meeting content.
pub async fn run<G, F, P>(lib: &Library, id: &str, generate: G, progress: P) -> Result<Value>
where
    G: Fn(services::Processing) -> F,
    F: Future<Output = Result<Value>>,
    P: Fn(&str),
{
    let meeting = store::load(&*crate::lock_db(lib)?, id)?;
    let state = &meeting["recordingProcessing"];
    let Some(generation) = state["generation"].as_str().map(str::to_owned) else {
        return Ok(meeting);
    };
    if ["complete", "skipped"].contains(&state["status"].as_str().unwrap_or("")) {
        return Ok(meeting);
    }
    mark(lib, id, &generation, "running", None)?;
    let result = async {
        let meeting = store::load(&*crate::lock_db(lib)?, id)?;
        if meeting["recordingProcessing"]["stage"] == "transcript" {
            progress("transcript");
            let mut job = services::prepare(lib, id, "transcript", None)?;
            let mut input = services::prepare(lib, id, "transcript", None)?;
            let ids = job.meeting["recordingProcessing"]["audioIds"]
                .as_array()
                .ok_or("Invalid recording processing audio")?;
            input.meeting["recordings"] = json!(input.meeting["recordings"]
                .as_array()
                .ok_or("Invalid recordings")?
                .iter()
                .filter(|r| ids.contains(&r["id"]))
                .collect::<Vec<_>>());
            if input.meeting["recordings"]
                .as_array()
                .is_none_or(|r| r.is_empty())
            {
                return Err("No finalized audio to process.".into());
            }
            let generated = generate(input).await?;
            let mut transcript = job.meeting["transcript"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            transcript.extend(
                generated
                    .as_array()
                    .ok_or("Invalid transcription result")?
                    .iter()
                    .cloned(),
            );
            transcript.sort_by(|a, b| {
                a["start"]
                    .as_f64()
                    .unwrap_or(0.0)
                    .total_cmp(&b["start"].as_f64().unwrap_or(0.0))
            });
            let transcript = json!(transcript);
            let no_new_speech = generated.as_array().into_iter().flatten().all(|segment| {
                segment["text"]
                    .as_str()
                    .is_none_or(|text| text.trim().is_empty())
            });
            let skip = if no_new_speech {
                Some("No new speech was transcribed. Audio and earlier content are saved.")
            } else {
                eligibility(&transcript)
            };
            if let Some(reason) = skip {
                job.meeting["recordingProcessing"]["stage"] = json!("complete");
                job.meeting["recordingProcessing"]["status"] = json!("skipped");
                job.meeting["recordingProcessing"]["error"] = json!(reason);
            } else {
                job.meeting["recordingProcessing"]["stage"] = json!("summary");
            }
            let saved = services::finish(lib, &job, transcript, None)?;
            if saved["recordingProcessing"]["status"] == "skipped" {
                return Ok(saved);
            }
        }
        progress("summary");
        let mut job = services::prepare(lib, id, "summary", None)?;
        let generated = generate(services::prepare(lib, id, "summary", None)?).await?;
        job.meeting["recordingProcessing"]["stage"] = json!("complete");
        job.meeting["recordingProcessing"]["status"] = json!("complete");
        job.meeting["recordingProcessing"]["error"] = Value::Null;
        services::finish(lib, &job, generated, None)
    }
    .await;
    if let Err(error) = &result {
        let _ = mark(lib, id, &generation, "failed", Some(error));
    }
    result
}
#[tauri::command]
pub async fn process_recording(app: tauri::AppHandle, id: String) -> Result<Value> {
    let jobs = app.state::<Jobs>();
    let _processing = jobs
        .0
        .try_lock()
        .map_err(|_| "Recording processing is already running.")?;
    let activity = app.state::<crate::activity::Activity>();
    let _activity = activity.job()?;
    if app
        .state::<crate::Runtime>()
        .capture
        .lock()
        .map_err(|_| "Capture lock failed")?
        .is_some()
    {
        return Err("Stop recording before processing.".into());
    }
    let lib = app.state::<Library>();
    let parakeet = crate::parakeet_helper(&app)?;
    run(
        &lib,
        &id,
        |job| {
            let parakeet = parakeet.clone();
            let activity = activity.inner().clone();
            async move { services::generate(&job, parakeet, activity).await }
        },
        |stage| {
            let _ = app.emit(
                "patter-recording-processing",
                json!({"id":id,"stage":stage}),
            );
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };
    fn fixture() -> (Library, String) {
        let root =
            std::env::temp_dir().join(format!("patter-pipeline-test-{}", uuid::Uuid::new_v4()));
        let lib = store::open(&root).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let meeting = json!({"id":id,"title":"Fixture linked meeting","createdAt":chrono::Utc::now().to_rfc3339(),"duration":8,"notes":"Keep notes","summary":"Previous summary","decisions":[],"actions":[],"transcript":[{"start":0,"text":"Earlier saved segment"}],"recordings":[{"id":"old","path":"old.wav","offset":0},{"id":"new","path":"new.wav","offset":8}],"archived":true,"revision":0,"eventId":"fixture-event"});
        let saved = store::save(&mut crate::lock_db(&lib).unwrap(), meeting).unwrap();
        queue(&lib, saved, &HashSet::from(["old".into()]), true).unwrap();
        (lib, id)
    }
    fn transcript() -> Value {
        json!([{"start":8,"text":"This synthetic transcript contains enough useful detail to summarize the meeting. We reviewed the recording flow and calendar refresh, agreed to preserve earlier notes, and assigned concrete follow up actions."}])
    }
    fn summary() -> Value {
        json!({"summary":"Fixture summary","decisions":["Keep notes"],"actions":["Verify refresh"],"summarySource":{"model":"fixture"}})
    }
    #[tokio::test]
    async fn final_audio_transcribes_before_summary_and_repeat_request_is_noop() {
        let (lib, id) = fixture();
        let steps = Mutex::new(Vec::new());
        let saved = run(
            &lib,
            &id,
            |job| {
                steps.lock().unwrap().push(job.kind.clone());
                if job.kind == "transcript" {
                    assert_eq!(job.meeting["recordings"].as_array().unwrap().len(), 1);
                    assert_eq!(job.meeting["recordings"][0]["id"], "new");
                }
                async move {
                    Ok(if job.kind == "transcript" {
                        transcript()
                    } else {
                        summary()
                    })
                }
            },
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(*steps.lock().unwrap(), ["transcript", "summary"]);
        assert_eq!(saved["eventId"], "fixture-event");
        assert_eq!(saved["archived"], true);
        assert_eq!(saved["notes"], "Keep notes");
        assert_eq!(saved["recordings"].as_array().unwrap().len(), 2);
        assert_eq!(saved["transcript"].as_array().unwrap().len(), 2);
        assert_eq!(saved["recordingProcessing"]["status"], "complete");
        run(
            &lib,
            &id,
            |_| async { panic!("completed generation must not run twice") },
            |_| {},
        )
        .await
        .unwrap();
        std::fs::remove_dir_all(&lib.root).unwrap();
    }
    #[tokio::test]
    async fn summary_failure_preserves_transcript_and_retry_resumes_after_reload() {
        let (lib, id) = fixture();
        run(
            &lib,
            &id,
            |job| async move {
                if job.kind == "transcript" {
                    Ok(transcript())
                } else {
                    Err("Local summary model unavailable".into())
                }
            },
            |_| {},
        )
        .await
        .unwrap_err();
        let failed = store::load(&crate::lock_db(&lib).unwrap(), &id).unwrap();
        assert_eq!(failed["recordingProcessing"]["stage"], "summary");
        assert_eq!(failed["recordingProcessing"]["status"], "failed");
        assert_eq!(failed["summary"], "Previous summary");
        let root = lib.root.clone();
        drop(lib);
        let lib = store::open(&root).unwrap();
        let resumed = run(
            &lib,
            &id,
            |job| async move {
                assert_eq!(job.kind, "summary");
                Ok(summary())
            },
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(resumed["transcript"].as_array().unwrap().len(), 2);
        std::fs::remove_dir_all(&lib.root).unwrap();
    }
    #[tokio::test]
    async fn failed_transcription_skips_summary_and_short_speech_is_saved_without_summary() {
        let (lib, id) = fixture();
        let calls = AtomicUsize::new(0);
        run(
            &lib,
            &id,
            |job| {
                calls.fetch_add(1, Ordering::SeqCst);
                async move {
                    assert_eq!(job.kind, "transcript");
                    Err("Model not downloaded".into())
                }
            },
            |_| {},
        )
        .await
        .unwrap_err();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let short = run(
            &lib,
            &id,
            |job| async move {
                assert_eq!(job.kind, "transcript");
                Ok(json!([{"start":8,"text":"Hello"}]))
            },
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(short["recordingProcessing"]["status"], "skipped");
        assert_eq!(short["summary"], "Previous summary");
        std::fs::remove_dir_all(&lib.root).unwrap();
    }
    #[tokio::test]
    async fn silence_keeps_earlier_summary_and_failed_audio_is_carried_into_next_generation() {
        let (lib, id) = fixture();
        let mut meeting = store::load(&crate::lock_db(&lib).unwrap(), &id).unwrap();
        meeting["transcript"] = transcript();
        store::save(&mut crate::lock_db(&lib).unwrap(), meeting).unwrap();
        let silent = run(
            &lib,
            &id,
            |job| async move {
                assert_eq!(job.kind, "transcript");
                Ok(json!([]))
            },
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(silent["summary"], "Previous summary");
        assert_eq!(silent["recordingProcessing"]["status"], "skipped");
        let mut meeting = silent;
        meeting["recordingProcessing"]["stage"] = json!("transcript");
        meeting["recordingProcessing"]["status"] = json!("failed");
        meeting["recordings"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"later","path":"later.wav","offset":10}));
        let saved = store::save(&mut crate::lock_db(&lib).unwrap(), meeting).unwrap();
        let queued = queue(
            &lib,
            saved,
            &HashSet::from(["old".into(), "new".into()]),
            true,
        )
        .unwrap();
        let ids = queued["recordingProcessing"]["audioIds"]
            .as_array()
            .unwrap();
        assert!(ids.contains(&json!("new")));
        assert!(ids.contains(&json!("later")));
        assert!(!ids.contains(&json!("old")));
        std::fs::remove_dir_all(&lib.root).unwrap();
    }
    #[test]
    fn empty_failed_or_unchanged_capture_does_not_queue_and_gate_rejects_concurrency() {
        let (lib, id) = fixture();
        let mut meeting = store::load(&crate::lock_db(&lib).unwrap(), &id).unwrap();
        meeting
            .as_object_mut()
            .unwrap()
            .remove("recordingProcessing");
        for (previous, successful) in [
            (HashSet::new(), false),
            (HashSet::from(["old".into(), "new".into()]), true),
        ] {
            assert!(queue(&lib, meeting.clone(), &previous, successful).unwrap()
                ["recordingProcessing"]
                .is_null());
        }
        meeting["recordings"] = json!([]);
        assert!(
            queue(&lib, meeting, &HashSet::new(), true).unwrap()["recordingProcessing"].is_null()
        );
        let jobs = Jobs::default();
        let guard = jobs.0.try_lock().unwrap();
        assert!(jobs.0.try_lock().is_err());
        drop(guard);
        assert!(jobs.0.try_lock().is_ok());
        std::fs::remove_dir_all(&lib.root).unwrap();
    }
}
