#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod activity;
mod models;
mod store;
mod updates;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Instant,
};
use store::{Library, Result};
use tauri::{Emitter, Manager, State};
struct Capture {
    child: Child,
    id: String,
    started: Instant,
    error: Arc<Mutex<Option<String>>>,
    _activity: activity::Guard,
}
struct Runtime {
    capture: Mutex<Option<Capture>>,
}
fn lock_db(lib: &Library) -> Result<std::sync::MutexGuard<'_, rusqlite::Connection>> {
    lib.db.lock().map_err(|_| "Library lock failed".into())
}
fn helper(app: &tauri::AppHandle) -> Result<PathBuf> {
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/patter-native");
    if cfg!(debug_assertions) && dev.exists() {
        return Ok(dev);
    }
    Ok(app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("resources/patter-native"))
}
#[tauri::command]
fn list_meetings(lib: State<Library>) -> Result<Vec<Value>> {
    store::all(&*lock_db(&lib)?)
}
#[tauri::command]
fn save_meeting(
    lib: State<Library>,
    activity: State<activity::Activity>,
    meeting: Value,
) -> Result<Value> {
    let _job = activity.job()?;
    store::save(&mut *lock_db(&lib)?, meeting)
}
#[tauri::command]
fn meeting_history(lib: State<Library>, id: String) -> Result<Vec<Value>> {
    let db = lock_db(&lib)?;
    let mut s = db
        .prepare("SELECT data FROM versions WHERE id=? ORDER BY revision DESC")
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([id], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string()))
        .collect()
}
#[tauri::command]
fn get_preferences(lib: State<Library>) -> Result<Value> {
    store::preferences(&*lock_db(&lib)?)
}
#[tauri::command]
fn set_preferences(
    lib: State<Library>,
    activity: State<activity::Activity>,
    preferences: Value,
) -> Result<()> {
    let _job = activity.job()?;
    models::local_url(preferences["endpoint"].as_str().unwrap_or(""))?;
    lock_db(&lib)?.execute("INSERT INTO preferences(id,data) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data",[preferences.to_string()]).map_err(|e|e.to_string())?;
    Ok(())
}
#[tauri::command]
async fn list_models(endpoint: String) -> Result<Vec<String>> {
    models::list(&endpoint).await
}
#[tauri::command]
async fn summarize(
    lib: State<'_, Library>,
    activity: State<'_, activity::Activity>,
    id: String,
) -> Result<Value> {
    let _job = activity.job()?;
    let (meeting, prefs) = {
        let db = lock_db(&lib)?;
        (store::load(&db, &id)?, store::preferences(&db)?)
    };
    let generated = models::summarize(&meeting, &prefs).await?;
    let mut db = lock_db(&lib)?;
    let mut latest = store::load(&db, &id)?;
    latest["summary"] = generated["summary"].clone();
    latest["decisions"] = generated["decisions"].clone();
    latest["actions"] = json!(generated["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a.as_str())
        .map(|text| json!({"id":uuid::Uuid::new_v4().to_string(),"text":text,"done":false}))
        .collect::<Vec<_>>());
    store::save(&mut db, latest)
}
fn audio_extension(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !["wav", "mp3", "m4a", "aiff", "aif", "caf", "flac", "ogg"].contains(&ext.as_str()) {
        return Err("Choose an audio file (WAV, MP3, M4A, AIFF, CAF, FLAC or OGG).".into());
    }
    Ok(ext)
}
#[tauri::command]
fn import_audio(
    lib: State<Library>,
    activity: State<activity::Activity>,
    id: String,
    path: String,
) -> Result<Value> {
    let _job = activity.job()?;
    store::valid_id(&id)?;
    let source = PathBuf::from(path);
    let ext = audio_extension(&source)?;
    if !source.is_file() {
        return Err("Audio file not found".into());
    }
    let mut db = lock_db(&lib)?;
    let mut m = store::load(&db, &id)?;
    let directory = lib.root.join("recordings").join(&id);
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let recording_id = uuid::Uuid::new_v4().to_string();
    let target = directory.join(format!("import-{recording_id}.{ext}"));
    // Durable copy is made before the library references it. Originals are never modified.
    let mut input = std::fs::File::open(&source).map_err(|e| e.to_string())?;
    let mut output = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&target)
        .map_err(|e| e.to_string())?;
    std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
    output.sync_all().map_err(|e| e.to_string())?;
    m["recordings"].as_array_mut().ok_or("Invalid recordings")?.push(json!({"id":recording_id,"name":source.file_name().unwrap_or_default().to_string_lossy(),"path":target,"track":"Imported audio","offset":0}));
    store::save(&mut db, m)
}
#[tauri::command]
async fn calendar_events(
    app: tauri::AppHandle,
    activity: State<'_, activity::Activity>,
) -> Result<Value> {
    let job = activity.job()?;
    let bin = helper(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _job = job;
        let output = Command::new(bin)
            .arg("calendar")
            .output()
            .map_err(|e| e.to_string())?;
        let data: Value = serde_json::from_slice(&output.stdout).map_err(|_| {
            format!(
                "Calendar access failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
        })?;
        if !output.status.success() {
            return Err(data["error"]
                .as_str()
                .unwrap_or("Calendar access failed")
                .to_string());
        }
        Ok(data)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn start_recording(
    app: tauri::AppHandle,
    lib: State<'_, Library>,
    runtime: State<'_, Runtime>,
    activity: State<'_, activity::Activity>,
    id: String,
) -> Result<()> {
    let job = activity.job()?;
    store::valid_id(&id)?;
    {
        let db = lock_db(&lib)?;
        store::load(&db, &id)?;
    }
    let directory = lib.root.join("recordings").join(&id);
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let receiver = {
        let mut guard = runtime.capture.lock().map_err(|_| "Capture lock failed")?;
        if guard.is_some() {
            return Err("A conversation is already being recorded.".into());
        }
        // Store the session marker before launching capture so recovery can find interrupted audio.
        std::fs::write(
            directory.join("capture.json"),
            json!({"id":id,"startedAt":chrono::Utc::now().to_rfc3339()}).to_string(),
        )
        .map_err(|e| e.to_string())?;
        let mut child = Command::new(helper(&app)?)
            .arg("record")
            .arg(&directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Capture status pipe unavailable")?;
        let error = Arc::new(Mutex::new(None));
        let errors = error.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut initialized = false;
            for line in BufReader::new(stdout)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if let Ok(v) = serde_json::from_str::<Value>(&line) {
                    if let Some(e) = v["error"].as_str() {
                        *errors.lock().unwrap() = Some(e.into());
                        if !initialized {
                            let _ = sender.send(Err(e.to_owned()));
                            initialized = true
                        }
                    }
                    if v["status"] == "recording" && !initialized {
                        let _ = sender.send(Ok(()));
                        initialized = true
                    }
                }
            }
            if !initialized {
                let _ = sender.send(Err(
                    "Audio capture could not start. Check macOS permissions.".into(),
                ));
            }
        });
        *guard = Some(Capture {
            child,
            id,
            started: Instant::now(),
            error,
            _activity: job,
        });
        receiver
    };
    let ready = tauri::async_runtime::spawn_blocking(move || {
        receiver
            .recv_timeout(std::time::Duration::from_secs(120))
            .map_err(|_| {
                "Audio capture did not start in time. Check macOS permissions.".to_string()
            })?
    })
    .await
    .map_err(|e| e.to_string())?;
    if ready.is_err() {
        if let Some(mut capture) = runtime
            .capture
            .lock()
            .map_err(|_| "Capture lock failed")?
            .take()
        {
            let _ = capture.child.kill();
            let _ = capture.child.wait();
        }
    }
    ready
}
#[tauri::command]
fn recording_status(runtime: State<Runtime>) -> Result<Value> {
    let mut guard = runtime.capture.lock().map_err(|_| "Capture lock failed")?;
    if let Some(c) = guard.as_mut() {
        let exit = c.child.try_wait().map_err(|e| e.to_string())?;
        let err = c
            .error
            .lock()
            .map_err(|_| "Capture status lock failed")?
            .clone();
        Ok(
            json!({"active":exit.is_none(),"id":c.id,"error":err.or_else(||exit.map(|_|"Recording stopped unexpectedly. Press Stop to recover the saved audio.".into()))}),
        )
    } else {
        Ok(json!({"active":false}))
    }
}
fn collect_audio(lib: &Library, id: &str, duration: Option<f64>) -> Result<Value> {
    let directory = lib.root.join("recordings").join(id);
    let mut db = lock_db(lib)?;
    let mut m = store::load(&db, id)?;
    let before = m.clone();
    let mut entries = std::fs::read_dir(&directory)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.extension().and_then(|s| s.to_str()) != Some("caf") {
            continue;
        }
        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !stem.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        let wav = path.with_extension("wav");
        if !wav.exists() {
            let temporary = path.with_extension(format!("{}.wav", uuid::Uuid::new_v4()));
            let converted = Command::new("/usr/bin/afconvert")
                .arg(&path)
                .arg(&temporary)
                .args(["-f", "WAVE", "-d", "LEI16"])
                .output();
            if converted.is_ok_and(|output| output.status.success()) {
                std::fs::rename(temporary, &wav).map_err(|e| e.to_string())?;
            }
        }
        let playable = if wav.exists() { wav } else { path.clone() };
        let recordings = m["recordings"]
            .as_array_mut()
            .ok_or("Invalid recording list")?;
        if recordings.iter().any(|r| r["id"] == stem) {
            continue;
        }
        let offset = stem
            .split('-')
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0)
            / 1000.0;
        let track = if stem.contains("microphone") {
            "Microphone"
        } else {
            "Computer audio"
        };
        recordings.push(json!({"id":stem,"name":format!("{} · {:02}:{:02}",track,offset as u64/60,offset as u64%60),"path":playable,"track":track,"offset":offset}));
    }
    if let Some(seconds) = duration {
        m["duration"] = json!(seconds)
    }
    if m == before {
        Ok(m)
    } else {
        store::save(&mut db, m)
    }
}
#[tauri::command]
async fn stop_recording(lib: State<'_, Library>, runtime: State<'_, Runtime>) -> Result<Value> {
    let mut capture = runtime
        .capture
        .lock()
        .map_err(|_| "Capture lock failed")?
        .take()
        .ok_or("No active recording")?;
    let duration = capture.started.elapsed().as_secs_f64();
    if let Some(mut stdin) = capture.child.stdin.take() {
        let _ = stdin.write_all(b"stop\n");
    }
    let id = capture.id.clone();
    let capture_activity = capture._activity;
    tauri::async_runtime::spawn_blocking(move || {
        let deadline = Instant::now() + std::time::Duration::from_secs(20);
        loop {
            match capture.child.try_wait() {
                Ok(Some(_)) => break,
                Err(_) => break,
                Ok(None) => {
                    if Instant::now() > deadline {
                        let _ = capture.child.kill();
                        let _ = capture.child.wait();
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    let result = collect_audio(&lib, &id, Some(duration));
    drop(capture_activity);
    result
}
#[tauri::command]
async fn transcribe(
    lib: State<'_, Library>,
    activity: State<'_, activity::Activity>,
    id: String,
) -> Result<Value> {
    let _outer_job = activity.job()?;
    let job = activity.job()?;
    let (meeting, prefs) = {
        let db = lock_db(&lib)?;
        (store::load(&db, &id)?, store::preferences(&db)?)
    };
    let root = lib.root.clone();
    let mid = id.clone();
    let segments=tauri::async_runtime::spawn_blocking(move||->Result<Vec<Value>>{
 let _job = job;
 let model=prefs["whisperModel"].as_str().filter(|p|Path::new(p).is_file()).ok_or("Choose a local Whisper .bin model in Settings first.")?;
 let context=whisper_rs::WhisperContext::new_with_params(model,whisper_rs::WhisperContextParameters::default()).map_err(|e|format!("Whisper could not load the selected model: {e}"))?;
 let recordings=meeting["recordings"].as_array().ok_or("No recordings")?;if recordings.is_empty(){return Err("Import or record some audio first.".into())}let mut segments=Vec::new();
 let working=root.join("processing").join(uuid::Uuid::new_v4().to_string());std::fs::create_dir_all(&working).map_err(|e|e.to_string())?;
 for (i,r) in recordings.iter().enumerate(){let input=PathBuf::from(r["path"].as_str().ok_or("Missing recording path")?);let canonical=input.canonicalize().map_err(|e|e.to_string())?;if !canonical.starts_with(root.join("recordings").join(&mid).canonicalize().map_err(|e|e.to_string())?){return Err("Recording path is outside this conversation.".into())}
 let wave=working.join(format!("{i}.wav"));let result=Command::new("/usr/bin/afconvert").arg(&input).arg(&wave).args(["-f","WAVE","-d","LEI16@16000","-c","1"]).output().map_err(|e|e.to_string())?;if !result.status.success(){return Err(format!("Could not prepare audio: {}",String::from_utf8_lossy(&result.stderr)))}
 let mut reader=hound::WavReader::open(&wave).map_err(|e|e.to_string())?;let samples=reader.samples::<i16>().map(|s|s.map(|x|x as f32/32768.0)).collect::<std::result::Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
 let mut state=context.create_state().map_err(|e|e.to_string())?;let mut params=whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy{best_of:1});params.set_language(None);params.set_translate(false);params.set_print_progress(false);params.set_print_realtime(false);params.set_print_timestamps(false);params.set_n_threads(4);
 state.full(params,&samples).map_err(|e|e.to_string())?;
 let offset=r["offset"].as_f64().unwrap_or(0.0);for s in state.as_iter(){segments.push(json!({"start":offset+s.start_timestamp() as f64/100.0,"text":s.to_str_lossy().map_err(|e|e.to_string())?.trim(),"speaker":r["track"].as_str().unwrap_or("Audio")}));}
 }
 segments.sort_by(|a,b|a["start"].as_f64().partial_cmp(&b["start"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));Ok(segments)
 }).await.map_err(|e|e.to_string())??;
    let mut db = lock_db(&lib)?;
    let mut latest = store::load(&db, &id)?;
    latest["transcript"] = json!(segments);
    store::save(&mut db, latest)
}
fn copy_tree(source: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let e = entry.map_err(|e| e.to_string())?;
        let target = dest.join(e.file_name());
        if e.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_tree(&e.path(), &target)?
        } else if e.file_type().map_err(|e| e.to_string())?.is_file() {
            std::fs::copy(e.path(), target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
#[tauri::command]
fn backup_library(
    lib: State<Library>,
    runtime: State<Runtime>,
    destination: String,
    activity: State<activity::Activity>,
) -> Result<String> {
    let _job = activity.exclusive()?;
    let capture = runtime.capture.lock().map_err(|_| "Capture lock failed")?;
    if capture.is_some() {
        return Err("Stop the recording before backing up the library.".into());
    }
    let parent = Path::new(&destination)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if parent.starts_with(&lib.root) {
        return Err("Choose a backup folder outside the Patter library.".into());
    }
    let destination = parent.join(format!(
        "Patter-backup-{}-{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        &uuid::Uuid::new_v4().to_string()[..8]
    ));
    std::fs::create_dir(&destination).map_err(|e| e.to_string())?;
    let db = lock_db(&lib)?;
    db.backup("main", destination.join("patter.sqlite3"), None)
        .map_err(|e| e.to_string())?;
    copy_tree(
        &lib.root.join("recordings"),
        &destination.join("recordings"),
    )?;
    std::fs::write(destination.join("README.txt"),"Patter complete library backup. Quit Patter before restoring. Keep a copy of the current library. Restore patter.sqlite3 and recordings/ to the same Patter application-support location. Recording paths are absolute in this initial version; moving to a different user account requires path migration.\n").map_err(|e|e.to_string())?;
    Ok(destination.to_string_lossy().into())
}
#[tauri::command]
fn finish_quit(
    app: tauri::AppHandle,
    runtime: State<Runtime>,
    activity: State<activity::Activity>,
) -> Result<()> {
    let _quit = activity.exclusive()?;
    if runtime
        .capture
        .lock()
        .map_err(|_| "Capture lock failed")?
        .is_some()
    {
        return Err("Stop recording before closing Patter.".into());
    }
    std::mem::forget(_quit); // Keep the gate closed until the exit event is handled.
    app.exit(0);
    Ok(())
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(activity::Activity::default())
        .manage(updates::Pending::default())
        .plugin(tauri_plugin_dialog::init())
        .manage(Runtime {
            capture: Mutex::new(None),
        })
        .setup(|app| {
            let root = app.path().app_data_dir()?;
            let library = store::open(&root).map_err(std::io::Error::other)?;
            // Recover surviving capture chunks on launch. This never deletes original audio.
            let ids = {
                let db = library.db.lock().unwrap();
                store::all(&db)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            };
            for id in ids {
                if library.root.join("recordings").join(&id).exists() {
                    let _ = collect_audio(&library, &id, None);
                }
            }
            app.manage(library);
            let menu = tauri::menu::Menu::default(app.handle())?;
            let check = tauri::menu::MenuItem::with_id(
                app,
                "check-updates",
                "Check for Updates…",
                true,
                None::<&str>,
            )?;
            if let Some(tauri::menu::MenuItemKind::Submenu(submenu)) = menu.items()?.first() {
                submenu.insert(&check, 1)?;
            }
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| {
                if event.id().as_ref() == "check-updates" {
                    let _ = app.emit("patter-check-updates", ());
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_meetings,
            save_meeting,
            meeting_history,
            get_preferences,
            set_preferences,
            list_models,
            summarize,
            import_audio,
            calendar_events,
            start_recording,
            stop_recording,
            recording_status,
            transcribe,
            backup_library,
            finish_quit,
            updates::check_update,
            updates::install_update
        ])
        .build(tauri::generate_context!())
        .expect("Patter could not start")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                    let _ = app.emit("patter-close-requested", ());
                }
            }
        });
}
