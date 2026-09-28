//! Local agent policy and tools. All mutation occurs in the running app.
#[cfg(test)]
mod tests;
mod transport;
use crate::{
    activity::Activity,
    services,
    store::{self, Library, Result},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};
use tauri::{Emitter, Manager};
pub use transport::mcp_main;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub allow_edit: bool,
    pub allow_processing: bool,
}
struct Control {
    config: Config,
    generation: u64,
    recent: Vec<Value>,
}
pub struct Access {
    control: Mutex<Control>,
    jobs: Mutex<BTreeMap<String, Value>>,
    dir: PathBuf,
    startup_error: Option<String>,
}
fn io(e: impl std::fmt::Display) -> String {
    e.to_string()
}
impl Access {
    fn new(dir: PathBuf) -> Result<Self> {
        transport::private_directory(&dir)?;
        let path = dir.join("settings.json");
        let config = if path.exists() {
            serde_json::from_slice(&std::fs::read(path).map_err(io)?).map_err(io)?
        } else {
            Config::default()
        };
        Ok(Self {
            control: Mutex::new(Control {
                config,
                generation: 0,
                recent: vec![],
            }),
            jobs: Mutex::new(BTreeMap::new()),
            dir,
            startup_error: None,
        })
    }
    fn authorize(control: &Control, tool: &str) -> Result<()> {
        if !control.config.enabled {
            return Err(
                "Agent access is off. Enable it in Patter → Settings → Agent access.".into(),
            );
        }
        if tool == "edit_conversation" && !control.config.allow_edit {
            return Err("Editing is off in Patter's Agent access settings.".into());
        }
        if tool == "start_processing" && !control.config.allow_processing {
            return Err("Local processing is off in Patter's Agent access settings.".into());
        }
        Ok(())
    }
    fn configure(&self, config: Config) -> Result<()> {
        if let Some(error) = &self.startup_error {
            return Err(error.clone());
        }
        let mut control = self.control.lock().map_err(|_| "Agent lock failed")?;
        transport::write_private(
            &self.dir.join("settings.json"),
            &serde_json::to_vec(&config).map_err(io)?,
        )?;
        control.config = config;
        control.generation += 1;
        Ok(())
    }
    fn audit(control: &mut Control, tool: &str, id: &str, outcome: &str) {
        control.recent.insert(0, json!({"tool":tool,"conversationId":id,"outcome":outcome,"at":chrono::Utc::now().to_rfc3339()}));
        control.recent.truncate(50);
    }
}
fn metadata(tool: &str) -> Value {
    json!({"source":"local-mcp","tool":tool,"at":chrono::Utc::now().to_rfc3339()})
}
fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| format!("Invalid arguments: {e}"))
}
fn page(offset: Option<usize>, limit: Option<usize>, max: usize) -> Result<(usize, usize)> {
    let limit = limit.unwrap_or(max);
    if limit == 0 || limit > max {
        return Err(format!("limit must be between 1 and {max}"));
    }
    Ok((offset.unwrap_or(0), limit))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    query: Option<String>,
    from: Option<String>,
    to: Option<String>,
    archived: Option<bool>,
    offset: Option<usize>,
    limit: Option<usize>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Read {
    id: String,
    section: String,
    revision: Option<i64>,
    offset: Option<usize>,
    limit: Option<usize>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    id: String,
    offset: Option<usize>,
    limit: Option<usize>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Edit {
    id: String,
    expected_revision: i64,
    title: Option<String>,
    notes: Option<String>,
    action_id: Option<String>,
    action_done: Option<bool>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Process {
    id: String,
    expected_revision: i64,
    kind: String,
    template: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Id {
    id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

/// Bounded read and edit operations, shared by protocol tests and the app host.
fn library_call(lib: &Library, access: &Access, tool: &str, args: Value) -> Result<Value> {
    let mut control = access.control.lock().map_err(|_| "Agent lock failed")?;
    Access::authorize(&control, tool)?;
    let mut db = lib.db.lock().map_err(|_| "Library lock failed")?;
    match tool {
        "search_conversations" => {
            let a: Search = parse(args)?;
            let (offset, limit) = page(a.offset, a.limit, 50)?;
            let query = a.query.unwrap_or_default().to_lowercase();
            if query.len() > 1000 {
                return Err("Keep search queries within 1,000 bytes.".into());
            }
            for date in [&a.from, &a.to].into_iter().flatten() {
                chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                    .map_err(|_| "Dates must use YYYY-MM-DD")?;
            }
            let words: Vec<_> = query.split_whitespace().collect();
            let mut matches = vec![];
            for m in store::all(&db)? {
                let date = m["createdAt"]
                    .as_str()
                    .unwrap_or("")
                    .get(..10)
                    .unwrap_or("");
                if a.from.as_ref().is_some_and(|d| date < d.as_str())
                    || a.to.as_ref().is_some_and(|d| date > d.as_str())
                    || a.archived
                        .is_some_and(|v| m["archived"].as_bool().unwrap_or(false) != v)
                {
                    continue;
                }
                let mut sections = vec![
                    m["title"].as_str().unwrap_or(""),
                    m["notes"].as_str().unwrap_or(""),
                    m["summary"].as_str().unwrap_or(""),
                ];
                if let Some(segments) = m["transcript"].as_array() {
                    sections.extend(segments.iter().filter_map(|s| s["text"].as_str()));
                }
                if let Some(actions) = m["actions"].as_array() {
                    sections.extend(actions.iter().filter_map(|s| s["text"].as_str()));
                }
                let text = sections.join(" ").to_lowercase();
                if !words.iter().all(|word| text.contains(word)) {
                    continue;
                }
                let excerpt = sections
                    .iter()
                    .find(|s| {
                        !words.is_empty() && words.iter().any(|w| s.to_lowercase().contains(w))
                    })
                    .copied()
                    .unwrap_or(sections[1]);
                matches.push(json!({"id":m["id"],"title":m["title"],"createdAt":m["createdAt"],"revision":m["revision"],"archived":m["archived"],"excerpt":excerpt.chars().take(240).collect::<String>()}));
            }
            let total = matches.len();
            Ok(
                json!({"total":total,"conversations":matches.into_iter().skip(offset).take(limit).collect::<Vec<_>>(),"nextOffset":(offset.saturating_add(limit)<total).then_some(offset.saturating_add(limit))}),
            )
        }
        "get_conversation" => {
            let a: Read = parse(args)?;
            store::valid_id(&a.id)?;
            let m: Value = if let Some(revision) = a.revision {
                let raw: String = db
                    .query_row(
                        "SELECT data FROM versions WHERE id=? AND revision=?",
                        rusqlite::params![a.id, revision],
                        |r| r.get(0),
                    )
                    .map_err(io)?;
                serde_json::from_str(&raw).map_err(io)?
            } else {
                store::load(&db, &a.id)?
            };
            let (offset, limit) = page(
                a.offset,
                a.limit,
                if a.section == "transcript" { 50 } else { 8000 },
            )?;
            let (content, total) = match a.section.as_str() {
                "notes" | "summary" => {
                    let chars: Vec<_> = m[&a.section].as_str().unwrap_or("").chars().collect();
                    (
                        json!(chars.iter().skip(offset).take(limit).collect::<String>()),
                        chars.len(),
                    )
                }
                "transcript" => {
                    let rows = m["transcript"].as_array().cloned().unwrap_or_default();
                    (json!(rows.iter().skip(offset).take(limit).map(|r|json!({"start":r["start"],"speaker":r["speaker"],"text":r["text"]})).collect::<Vec<_>>()),rows.len())
                }
                "overview" => (
                    json!({"createdAt":m["createdAt"],"duration":m["duration"],"archived":m["archived"],"actions":m["actions"],"decisions":m["decisions"],"summarySource":m["summarySource"],"recordingCount":m["recordings"].as_array().map_or(0,Vec::len),"agentChange":m["agentChange"]}),
                    0,
                ),
                _ => return Err("Choose overview, notes, summary or transcript.".into()),
            };
            Ok(
                json!({"id":m["id"],"title":m["title"],"revision":m["revision"],"section":a.section,"content":content,"total":total,"nextOffset":(offset.saturating_add(limit)<total).then_some(offset.saturating_add(limit))}),
            )
        }
        "conversation_history" => {
            let a: History = parse(args)?;
            store::valid_id(&a.id)?;
            let (offset, limit) = page(a.offset, a.limit, 50)?;
            let mut stmt=db.prepare("SELECT revision,saved_at,json_extract(data,'$.agentChange') FROM versions WHERE id=? ORDER BY revision DESC LIMIT ? OFFSET ?").map_err(io)?;
            let rows=stmt.query_map(rusqlite::params![a.id,limit as i64,offset.min(i64::MAX as usize) as i64],|row|Ok(json!({"revision":row.get::<_,i64>(0)?,"savedAt":row.get::<_,String>(1)?,"agentChange":row.get::<_,Option<String>>(2)?.and_then(|s|serde_json::from_str::<Value>(&s).ok())}))).map_err(io)?.collect::<std::result::Result<Vec<_>,_>>().map_err(io)?;
            Ok(json!({"versions":rows,"offset":offset}))
        }
        "edit_conversation" => {
            let a: Edit = parse(args)?;
            if a.title.is_none() && a.notes.is_none() && a.action_id.is_none() {
                return Err("Supply a title, notes, or action completion change.".into());
            }
            let mut m = store::load(&db, &a.id)?;
            if m["revision"].as_i64() != Some(a.expected_revision) {
                return Err(
                    "REVISION_CONFLICT: Read the latest conversation and retry with its revision."
                        .into(),
                );
            }
            if let Some(title) = a.title {
                if title.trim().is_empty() || title.chars().count() > 300 {
                    return Err("Title must contain 1–300 characters.".into());
                }
                m["title"] = json!(title);
            }
            if let Some(notes) = a.notes {
                if notes.len() > 48_000 {
                    return Err("Keep notes within 48,000 bytes per agent edit.".into());
                }
                m["notes"] = json!(notes);
            }
            match (a.action_id, a.action_done) {
                (Some(id), Some(done)) => {
                    let action = m["actions"]
                        .as_array_mut()
                        .ok_or("No actions")?
                        .iter_mut()
                        .find(|a| a["id"] == id)
                        .ok_or("Action not found")?;
                    action["done"] = json!(done);
                }
                (None, None) => {}
                _ => return Err("Supply both action_id and action_done.".into()),
            }
            let saved = store::save_checked_attributed(&mut db, m, Some(metadata(tool)))?;
            Access::audit(&mut control, tool, &a.id, "saved");
            Ok(json!({"id":saved["id"],"revision":saved["revision"]}))
        }
        "list_templates" => {
            let _: Empty = parse(args)?;
            let builtins: Vec<Value> =
                serde_json::from_str(include_str!("../../../src/lib/summary-templates.json"))
                    .map_err(io)?;
            Ok(
                json!({"templates":builtins.iter().map(|t|json!({"id":t["id"],"name":t["name"]})).collect::<Vec<_>>()}),
            )
        }
        _ => Err("Unknown tool".into()),
    }
}

pub fn start(app: &tauri::AppHandle) {
    let result = transport::default_directory().and_then(|dir| {
        let access = Access::new(dir.clone())?;
        transport::start_host(app.clone(), dir.join("agent.sock"))?;
        Ok(access)
    });
    let access = result.unwrap_or_else(|error| Access {
        control: Mutex::new(Control {
            config: Config::default(),
            generation: 0,
            recent: vec![],
        }),
        jobs: Mutex::new(BTreeMap::new()),
        dir: PathBuf::new(),
        startup_error: Some(error),
    });
    app.manage(access);
}
#[tauri::command]
pub fn agent_configure(app: tauri::AppHandle, config: Config) -> Result<()> {
    let _guard = app.state::<Activity>().job()?;
    app.state::<Access>().configure(config)
}
#[tauri::command]
pub fn agent_status(app: tauri::AppHandle) -> Result<Value> {
    let access = app.state::<Access>();
    let control = access.control.lock().map_err(|_| "Agent lock failed")?;
    let executable = std::env::current_exe().map_err(io)?;
    Ok(
        json!({"error":access.startup_error,"config":control.config,"recent":control.recent,"jobs":access.jobs.lock().map_err(|_|"Job lock failed")?.values().collect::<Vec<_>>(),"connection":{"mcpServers":{"patter":{"command":executable,"args":["--mcp","--socket",access.dir.join("agent.sock")]}}}}),
    )
}
fn finish_processing(
    lib: &Library,
    access: &Access,
    prepared: &services::Processing,
    output: Value,
    generation: u64,
) -> Result<Value> {
    let mut control = access.control.lock().map_err(|_| "Agent lock failed")?;
    Access::authorize(&control, "start_processing")?;
    if control.generation != generation {
        return Err("Agent permissions changed while processing. Result was not saved; start a new job if wanted.".into());
    }
    let saved = services::finish(lib, prepared, output, Some(metadata("start_processing")))?;
    Access::audit(
        &mut control,
        "start_processing",
        prepared.meeting["id"].as_str().unwrap_or(""),
        "saved",
    );
    Ok(saved)
}
async fn dispatch(app: tauri::AppHandle, tool: String, args: Value) -> Result<Value> {
    let _guard = app.state::<Activity>().job()?;
    let access = app.state::<Access>();
    {
        Access::authorize(
            &*access.control.lock().map_err(|_| "Agent lock failed")?,
            &tool,
        )?;
    }
    match tool.as_str() {
        "start_processing" => {
            let a: Process = parse(args)?;
            if crate::recording_status(app.state())?["active"] == true {
                return Err("Finish recording before starting agent processing.".into());
            }
            let prepared = services::prepare(
                &app.state::<Library>(),
                &a.id,
                &a.kind,
                a.template.as_deref(),
            )?;
            if prepared.meeting["revision"].as_i64() != Some(a.expected_revision) {
                return Err(
                    "REVISION_CONFLICT: Read the latest conversation before processing.".into(),
                );
            }
            let parakeet = crate::parakeet_helper(&app)?;
            let job_id = uuid::Uuid::new_v4().to_string();
            let generation = {
                let mut c = access.control.lock().map_err(|_| "Agent lock failed")?;
                Access::authorize(&c, &tool)?;
                let mut jobs = access.jobs.lock().map_err(|_| "Job lock failed")?;
                if jobs.values().any(|j| j["status"] == "running") {
                    return Err("An agent processing job is already running. Poll job_status before starting another.".into());
                }
                if jobs.len() >= 100 {
                    jobs.clear();
                }
                jobs.insert(job_id.clone(),json!({"id":job_id,"conversationId":a.id,"kind":a.kind,"status":"running","stage":"processing locally","startedAt":chrono::Utc::now().to_rfc3339()}));
                Access::audit(&mut c, &tool, &a.id, "started");
                c.generation
            };
            let worker = app.clone();
            let id = job_id.clone();
            tauri::async_runtime::spawn(async move {
                let _held = _guard;
                let output = services::generate(
                    &prepared,
                    parakeet,
                    worker.state::<Activity>().inner().clone(),
                )
                .await;
                let access = worker.state::<Access>();
                let result = output.and_then(|output| {
                    finish_processing(
                        &worker.state::<Library>(),
                        &access,
                        &prepared,
                        output,
                        generation,
                    )
                });
                if let Ok(mut jobs) = access.jobs.lock() {
                    if let Some(job) = jobs.get_mut(&id) {
                        job["status"] = json!(if result.is_ok() {
                            "completed"
                        } else {
                            "failed"
                        });
                        job["stage"] = json!("finished");
                        job["finishedAt"] = json!(chrono::Utc::now().to_rfc3339());
                        match &result {
                            Ok(saved) => job["revision"] = saved["revision"].clone(),
                            Err(e) => job["error"] = json!(e),
                        }
                    }
                }
                if result.is_ok() {
                    let _ = worker.emit("patter-agent-changed", &a.id);
                }
            });
            Ok(json!({"jobId":job_id,"status":"running"}))
        }
        "job_status" => {
            let a: Id = parse(args)?;
            access
                .jobs
                .lock()
                .map_err(|_| "Job lock failed")?
                .get(&a.id)
                .cloned()
                .ok_or("Job not found. Jobs are retained for this app session.".into())
        }
        "open_conversation" => {
            let a: Id = parse(args)?;
            store::load(&*crate::lock_db(&app.state::<Library>())?, &a.id)?;
            if let Some(window) = app.get_webview_window("main") {
                window.show().map_err(io)?;
                window.set_focus().map_err(io)?;
            }
            app.emit("patter-agent-open", &a.id).map_err(io)?;
            Ok(json!({"requested":a.id}))
        }
        "patter_status" => {
            let _: Empty = parse(args)?;
            let recording = crate::recording_status(app.state())?;
            let root = &app.state::<Library>().root;
            // Never probe Keychain or expose account, credential, folder or filesystem identifiers.
            let backup: Value = std::fs::read(root.join("drive-backup-status.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or(json!({}));
            Ok(
                json!({"version":env!("CARGO_PKG_VERSION"),"recording":{"active":recording["active"],"conversationId":recording["id"]},"backup":{"phase":backup["phase"],"lastSuccess":backup["lastSuccess"],"completedFiles":backup["completedFiles"],"totalFiles":backup["totalFiles"]}}),
            )
        }
        _ => {
            let result = library_call(&app.state::<Library>(), &access, &tool, args)?;
            if tool == "edit_conversation" {
                let _ = app.emit("patter-agent-changed", result["id"].clone());
            }
            Ok(result)
        }
    }
}
