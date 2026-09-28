//! Shared UI/agent processing. Results may only commit against their source revision.
use crate::{models, store, templates, transcription};
use serde_json::{json, Value};
use std::path::PathBuf;
use store::{Library, Result};

pub struct Processing {
    pub meeting: Value,
    pub preferences: Value,
    pub root: PathBuf,
    pub kind: String,
}
pub fn prepare(lib: &Library, id: &str, kind: &str, template: Option<&str>) -> Result<Processing> {
    if !["summary", "transcript"].contains(&kind) {
        return Err("Choose summary or transcript.".into());
    }
    let db = lib.db.lock().map_err(|_| "Library lock failed")?;
    let mut meeting = store::load(&db, id)?;
    let preferences = store::preferences(&db)?;
    if let Some(template) = template {
        if kind != "summary" {
            return Err("Templates apply to summaries only.".into());
        }
        meeting["summaryTemplate"] = json!(template);
    }
    if kind == "summary" {
        templates::resolve(&meeting, &preferences)?;
    }
    Ok(Processing {
        meeting,
        preferences,
        root: lib.root.clone(),
        kind: kind.into(),
    })
}
pub async fn generate(
    job: &Processing,
    parakeet: PathBuf,
    activity: crate::activity::Activity,
) -> Result<Value> {
    if job.kind == "summary" {
        models::summarize(&job.meeting, &job.preferences).await
    } else {
        let guard = activity.job()?;
        let (root, meeting, preferences) = (
            job.root.clone(),
            job.meeting.clone(),
            job.preferences.clone(),
        );
        tauri::async_runtime::spawn_blocking(move || {
            let _guard = guard;
            transcription::transcribe(&root, &meeting, &preferences, &parakeet)
        })
        .await
        .map_err(|e| e.to_string())?
        .map(|segments| json!(segments))
    }
}
pub fn finish(
    lib: &Library,
    job: &Processing,
    generated: Value,
    agent: Option<Value>,
) -> Result<Value> {
    let mut meeting = job.meeting.clone();
    if job.kind == "summary" {
        meeting["summary"] = generated["summary"].clone();
        meeting["summarySource"] = generated["summarySource"].clone();
        meeting["decisions"] = generated["decisions"].clone();
        meeting["actions"] = json!(generated["actions"]
            .as_array()
            .ok_or("Invalid model actions")?
            .iter()
            .filter_map(Value::as_str)
            .map(|text| json!({"id":uuid::Uuid::new_v4().to_string(),"text":text,"done":false}))
            .collect::<Vec<_>>());
    } else {
        meeting["transcript"] = generated;
    }
    let mut db = lib.db.lock().map_err(|_| "Library lock failed")?;
    store::save_checked_attributed(&mut db, meeting, agent)
}
