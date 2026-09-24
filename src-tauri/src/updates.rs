use crate::{
    activity::Activity,
    lock_db,
    store::{self, Library, Result},
};
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);
#[tauri::command]
pub async fn check_update(app: AppHandle, pending: State<'_, Pending>) -> Result<Value> {
    let update = app
        .updater_builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let metadata = update
        .as_ref()
        .map(|u| json!({"version":u.version,"notes":u.body}));
    *pending.0.lock().map_err(|_| "Update lock failed")? = update;
    Ok(json!({"currentVersion":app.package_info().version.to_string(),"update":metadata}))
}
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    pending: State<'_, Pending>,
    activity: State<'_, Activity>,
    lib: State<'_, Library>,
    version: String,
) -> Result<()> {
    // Frontend flushes pending edits first. This guard atomically excludes all native work
    // for download, snapshot, replacement and restart, and releases on any error.
    let _install = activity.exclusive()?;
    let update = {
        let state = pending.0.lock().map_err(|_| "Update lock failed")?;
        let update = state.as_ref().ok_or("Check for updates again first.")?;
        if update.version != version {
            return Err("The available release changed. Check for updates again.".into());
        }
        update.clone()
    };
    let mut downloaded = 0_u64;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = app.emit(
                    "patter-update-progress",
                    json!({"downloaded":downloaded,"total":total}),
                );
            },
            || {},
        )
        .await
        .map_err(|e| format!("Update download failed: {e}"))?;
    // download() verifies the signature before returning. Backup failure prevents install.
    store::snapshot(
        &*lock_db(&lib)?,
        &lib.root,
        &format!("before-update-{}", update.version),
    )?;
    let _ = app.emit("patter-update-progress", json!({"installing":true}));
    update
        .install(bytes)
        .map_err(|e| format!("Update could not be installed: {e}"))?;
    app.restart();
}
