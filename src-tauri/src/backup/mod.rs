mod google;
mod schedule;
pub mod snapshot;
use crate::{activity::Activity, store::Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use tauri::Manager;

pub(super) fn io(e: impl std::fmt::Display) -> String {
    e.to_string()
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub enabled: bool,
    pub time: String,
    pub device_id: String,
    pub library_id: String,
    pub folder_id: String,
    pub email: String,
    pub connected: bool,
    pub client_id: String,
    pub credential_account: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            time: "02:00".into(),
            device_id: uuid::Uuid::new_v4().to_string(),
            library_id: uuid::Uuid::new_v4().to_string(),
            folder_id: String::new(),
            email: String::new(),
            connected: false,
            client_id: String::new(),
            credential_account: String::new(),
        }
    }
}
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Status {
    pub phase: String,
    pub message: String,
    pub last_success: Option<String>,
    pub last_snapshot: Option<String>,
    pub last_attempt: Option<String>,
    pub completed_files: usize,
    pub total_files: usize,
    pub uploaded_bytes: u64,
    pub pending: Option<String>,
}
fn config(root: &Path) -> Result<Config> {
    let path = root.join("drive-backup.json");
    if path.exists() {
        read_json(&path)
    } else {
        let c = Config::default();
        atomic_json(&path, &c)?;
        Ok(c)
    }
}
fn status(root: &Path) -> Result<Status> {
    let p = root.join("drive-backup-status.json");
    if p.exists() {
        read_json(&p)
    } else {
        Ok(Status::default())
    }
}
fn read_json<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T> {
    serde_json::from_slice(&fs::read(p).map_err(io)?).map_err(io)
}
pub(super) fn atomic_json(p: &Path, v: &impl Serialize) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::create_dir_all(p.parent().ok_or("Invalid state path")?).map_err(io)?;
    let tmp = p.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut f = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(io)?;
    f.write_all(&serde_json::to_vec_pretty(v).map_err(io)?)
        .map_err(io)?;
    f.sync_all().map_err(io)?;
    fs::rename(tmp, p).map_err(io)?;
    fs::File::open(p.parent().unwrap())
        .map_err(io)?
        .sync_all()
        .map_err(io)
}
pub(super) fn hash_file(p: &Path) -> Result<String> {
    let mut f = fs::File::open(p).map_err(io)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf).map_err(io)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
pub(super) fn safe_relative(p: &Path) -> Result<()> {
    if p.as_os_str().is_empty() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        Err("Unsafe backup path".into())
    } else {
        Ok(())
    }
}
fn lock_path(root: &Path, name: &str) -> Result<PathBuf> {
    if !matches!(name, "activity" | "backup" | "app") {
        return Err("Invalid lock name".into());
    }
    let key = format!("{:x}", Sha256::digest(root.to_string_lossy().as_bytes()));
    let dir = root
        .parent()
        .ok_or("Invalid library path")?
        .join(".patter-locks");
    fs::create_dir_all(&dir).map_err(io)?;
    Ok(dir.join(format!("{key}-{name}.lock")))
}
pub fn process_lock(root: &Path, name: &str, exclusive: bool) -> Result<fs::File> {
    let f = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path(root, name)?)
        .map_err(io)?;
    let locked = if exclusive {
        f.try_lock_exclusive()
    } else {
        FileExt::try_lock_shared(&f)
    };
    locked.map_err(|_| {
        "Patter is busy in another process. It will retry when the current operation finishes."
            .to_string()
    })?;
    Ok(f)
}
fn write_status(root: &Path, s: &Status) -> Result<()> {
    atomic_json(&root.join("drive-backup-status.json"), s)
}
fn root_from(app: &tauri::AppHandle) -> PathBuf {
    app.state::<crate::store::Library>().root.clone()
}
pub fn stable_root() -> Result<PathBuf> {
    Ok(
        PathBuf::from(std::env::var_os("HOME").ok_or("Home folder unavailable")?)
            .join("Library/Application Support/gr.tau.patter"),
    )
}

fn run(root: &Path, activity: &Activity) -> Result<Value> {
    let _run = process_lock(root, "backup", true)?;
    let c = config(root)?;
    if !c.connected || c.folder_id.is_empty() {
        return Err("Connect Google Drive before backing up.".into());
    }
    let mut state = status(root)?;
    state.last_attempt = Some(chrono::Utc::now().to_rfc3339());
    state.phase = "preparing".into();
    state.message = "Preparing your backup…".into();
    write_status(root, &state)?;
    let work = (|| -> Result<Value> {
        let staging = if let Some(id) = &state.pending {
            crate::store::valid_id(id)?;
            root.join("backup-staging").join(id)
        } else {
            let _capture = activity.exclusive()?;
            let id = uuid::Uuid::new_v4().to_string();
            let staging = root.join("backup-staging").join(&id);
            fs::create_dir_all(staging.parent().unwrap()).map_err(io)?;
            if let Err(e) = snapshot::capture(root, &staging, &c) {
                // Only incomplete derived copies are discarded. Source files are untouched.
                let _ = fs::remove_dir_all(&staging);
                return Err(e);
            }
            state.pending = Some(id);
            write_status(root, &state)?;
            staging
        };
        let _upload = activity.job()?;
        let mut manifest: snapshot::Manifest = read_json(&staging.join("manifest.json"))?;
        snapshot::validate(&manifest)?;
        let mut drive = google::Drive::new(&c)?;
        let destination_key = format!(
            "{:x}",
            Sha256::digest(format!("{}:{}:{}", c.client_id, c.email, c.folder_id).as_bytes())
        );
        let checkpoints = staging.join("checkpoints").join(destination_key);
        state.phase = "uploading".into();
        state.total_files = manifest.entries.len();
        state.completed_files = 0;
        state.uploaded_bytes = 0;
        let folder = drive.folder(
            &c.folder_id,
            &format!("Mac {}", &c.device_id.chars().take(8).collect::<String>()),
            &format!("device-{}", c.device_id),
        )?;
        let folder = drive.folder(
            &folder,
            &format!(
                "Library {}",
                &c.library_id.chars().take(8).collect::<String>()
            ),
            &format!("library-{}", c.library_id),
        )?;
        for i in 0..manifest.entries.len() {
            let entry = &manifest.entries[i];
            safe_relative(Path::new(&entry.path))?;
            if hash_file(&staging.join(&entry.path))? != entry.sha256 {
                return Err("A staged backup file failed verification. Preserve the staging folder and create a new backup after checking disk health.".into());
            }
            state.message = format!("Uploading file {} of {}", i + 1, state.total_files);
            write_status(root, &state)?;
            let mut parent = folder.clone();
            let path = Path::new(&entry.path);
            if let Some(parts) = path.parent() {
                for part in parts.components() {
                    let name = part.as_os_str().to_string_lossy();
                    parent = drive.folder(&parent, &name, &format!("path-{name}"))?;
                }
            }
            let name = path.file_name().unwrap().to_string_lossy();
            let id = drive.upload(
                &parent,
                &name,
                &staging.join(&entry.path),
                &entry.sha256,
                &checkpoints.join(format!("upload-{i}.json")),
            )?;
            manifest.entries[i].drive_id = id;
            state.completed_files = i + 1;
            state.uploaded_bytes += manifest.entries[i].size;
            write_status(root, &state)?;
        }
        let manifests = drive.folder(&folder, "snapshots", "snapshots")?;
        atomic_json(&staging.join("remote-manifest.json"), &manifest)?;
        let p = staging.join("remote-manifest.json");
        let id = drive.upload(
            &manifests,
            &format!(
                "{}--{}.json",
                manifest.created_at.replace(':', "-"),
                manifest.id
            ),
            &p,
            &hash_file(&p)?,
            &checkpoints.join("manifest-upload.json"),
        )?;
        // Only a fully uploaded and verified manifest constitutes success.
        state.phase = "complete".into();
        state.message = "Backup verified in Google Drive.".into();
        state.last_success = Some(chrono::Utc::now().to_rfc3339());
        state.last_snapshot = Some(manifest.created_at.clone());
        state.pending = None;
        write_status(root, &state)?;
        // These are verified temporary upload copies, never originals or retained backups.
        let _ = fs::remove_dir_all(&staging);
        Ok(json!({"snapshotId":id,"files":state.total_files}))
    })();
    if let Err(e) = &work {
        state.phase = "error".into();
        state.message = e.clone();
        let _ = write_status(root, &state);
    }
    work
}

#[tauri::command]
pub async fn backup_status(app: tauri::AppHandle) -> Result<Value> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = root_from(&app);
        let c = config(&root)?;
        let state = status(&root)?;
        let running = process_lock(&root, "backup", true).is_err();
        let needs_reconnect = c.connected && !running && !google::credentials_available(&c);
        Ok(json!({"config":c,"status":state,"needsReconnect":needs_reconnect,"running":running,"schedule":schedule::status(&root),"restoreReady":journal(&root)?.exists(),"timezone":chrono::Local::now().format("%Z %:z").to_string()}))
    }).await.map_err(io)?
}
#[tauri::command]
pub async fn backup_connect(app: tauri::AppHandle, credentials_path: Option<String>) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = root_from(&app);
        let _run = process_lock(&root, "backup", true)?;
        let _job = app.state::<Activity>().job()?;
        let mut c = config(&root)?;
        google::connect(&mut c, credentials_path.as_deref().map(Path::new))?;
        atomic_json(&root.join("drive-backup.json"), &c)?;
        let mut state = status(&root)?;
        state.phase = "idle".into();
        state.message = "Google Drive connected. Your backup can resume.".into();
        state.last_attempt = None;
        write_status(&root, &state)?;
        // Release both locks before RunAtLoad can launch the replacement worker.
        drop(_job);
        drop(_run);
        // Re-register the existing schedule so a pending backup can retry now.
        schedule::install(&root, &c)
    })
    .await
    .map_err(io)?
}
#[tauri::command]
pub async fn backup_now(app: tauri::AppHandle) -> Result<Value> {
    tauri::async_runtime::spawn_blocking(move || run(&root_from(&app), &app.state::<Activity>()))
        .await
        .map_err(io)?
}
#[tauri::command]
pub async fn backup_configure(app: tauri::AppHandle, enabled: bool, time: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = root_from(&app);
        let _run = process_lock(&root, "backup", true)?;
        let mut c = config(&root)?;
        schedule::parse_time(&time)?;
        if enabled && !c.connected {
            return Err("Connect Google Drive first.".into());
        }
        c.time = time;
        c.enabled = enabled;
        let previous = config(&root)?;
        atomic_json(&root.join("drive-backup.json"), &c)?;
        if let Err(e) = schedule::install(&root, &c) {
            let _ = atomic_json(&root.join("drive-backup.json"), &previous);
            let _ = schedule::install(&root, &previous);
            return Err(e);
        }
        Ok(())
    })
    .await
    .map_err(io)?
}
#[tauri::command]
pub async fn backup_disconnect(app: tauri::AppHandle) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = root_from(&app);
        let _run = process_lock(&root, "backup", true)?;
        let mut c = config(&root)?;
        c.enabled = false;
        schedule::install(&root, &c)?;
        google::disconnect(&c)?;
        c.connected = false;
        c.email.clear();
        atomic_json(&root.join("drive-backup.json"), &c)
    })
    .await
    .map_err(io)?
}
#[tauri::command]
pub fn backup_open_folder(app: tauri::AppHandle) -> Result<()> {
    let c = config(&root_from(&app))?;
    google::valid_id(&c.folder_id)?;
    std::process::Command::new("/usr/bin/open")
        .arg(format!(
            "https://drive.google.com/drive/folders/{}",
            c.folder_id
        ))
        .spawn()
        .map_err(io)?;
    Ok(())
}
#[tauri::command]
pub async fn backup_list_snapshots(app: tauri::AppHandle) -> Result<Value> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = root_from(&app);
        let _run = process_lock(&root, "backup", true)?;
        let _job = app.state::<Activity>().job()?;
        let c = config(&root)?;
        google::Drive::new(&c)?.snapshots()
    })
    .await
    .map_err(io)?
}

// Restore journal lives beside the library so it survives atomic directory replacement.
fn journal(root: &Path) -> Result<PathBuf> {
    Ok(root
        .parent()
        .ok_or("Invalid library path")?
        .join(".patter-pending-restore.json"))
}
#[tauri::command]
pub async fn backup_prepare_restore(app: tauri::AppHandle, snapshot_id: String) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move||{
        let root=root_from(&app);let _run=process_lock(&root,"backup",true)?;let _job=app.state::<Activity>().exclusive()?;
        if journal(&root)?.exists(){return Err("A verified restore is already ready. Restart Patter to finish it.".into());}
        let c=config(&root)?;let mut drive=google::Drive::new(&c)?;let id=uuid::Uuid::new_v4().to_string();
        let staging=root.join("restore-downloads").join(&id);fs::create_dir_all(&staging).map_err(io)?;
        let manifest=drive.download_snapshot(&snapshot_id,&staging)?;
        let target=root.parent().unwrap().join(format!("Patter-restored-{id}"));
        snapshot::restore(&staging,&target,&root,&manifest)?;
        let safety=root.parent().unwrap().join(format!("Patter-before-restore-{id}"));
        atomic_json(&journal(&root)?,&json!({"target":target,"safety":safety}))?;
        Ok("Restore verified. Restart Patter to open it. Your current library will be kept in a separate safety folder; reconnect Drive afterward.".into())
    }).await.map_err(io)?
}
#[tauri::command]
pub fn backup_restart(app: tauri::AppHandle) -> Result<()> {
    let root = root_from(&app);
    if !journal(&root)?.exists() {
        return Err("Prepare a verified restore first.".into());
    }
    let guard = app.state::<Activity>().exclusive()?;
    std::mem::forget(guard);
    app.restart()
}
fn replace_library(root: &Path, target: &Path, safety: &Path) -> Result<()> {
    let parent = root.parent().ok_or("Invalid library path")?;
    if target.exists() {
        if root.exists() {
            if safety.exists() {
                return Err(
                    "Both current and safety libraries exist. Restore needs review.".into(),
                );
            }
            fs::rename(root, safety).map_err(io)?;
            fs::File::open(parent).map_err(io)?.sync_all().map_err(io)?;
        }
        if let Err(e) = fs::rename(target, root) {
            if !root.exists() && safety.exists() {
                let _ = fs::rename(safety, root);
            }
            return Err(io(e));
        }
        fs::File::open(parent).map_err(io)?.sync_all().map_err(io)?;
    } else if !root.exists() || !safety.exists() {
        return Err("Restore files are missing; the previous library is preserved.".into());
    }
    Ok(())
}
pub fn apply_restore(root: &Path) -> Result<()> {
    let j = journal(root)?;
    if !j.exists() {
        return Ok(());
    }
    let _run = process_lock(root, "backup", true)?;
    let _activity = process_lock(root, "activity", true)?;
    let value: Value = read_json(&j)?;
    let parent = root.parent().unwrap();
    let target = PathBuf::from(value["target"].as_str().ok_or("Invalid restore journal")?);
    let safety = PathBuf::from(value["safety"].as_str().ok_or("Invalid restore journal")?);
    let mut restore_ids = Vec::new();
    for (path, prefix) in [
        (&target, "Patter-restored-"),
        (&safety, "Patter-before-restore-"),
    ] {
        if path.parent() != Some(parent) {
            return Err("Invalid restore destination".into());
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid restore destination")?;
        let id = name
            .strip_prefix(prefix)
            .ok_or("Invalid restore destination")?;
        crate::store::valid_id(id)?;
        restore_ids.push(id.to_owned());
    }
    if restore_ids[0] != restore_ids[1] {
        return Err("Restore destinations do not match".into());
    }
    if target.exists() {
        if fs::symlink_metadata(&target)
            .map_err(io)?
            .file_type()
            .is_symlink()
        {
            return Err("Invalid restored library".into());
        }
        let mut paused = config(&target)?;
        paused.enabled = false;
        schedule::install(root, &paused)?;
    }
    replace_library(root, &target, &safety)?;
    // Keep the journal as history without applying it twice.
    fs::rename(
        &j,
        parent.join(format!(
            ".patter-restore-completed-{}.json",
            uuid::Uuid::new_v4()
        )),
    )
    .map_err(io)?;
    fs::File::open(parent).map_err(io)?.sync_all().map_err(io)?;
    let c = config(root)?;
    schedule::install(root, &c)?;
    Ok(())
}
pub fn worker() -> Result<()> {
    // Each credential operation suppresses prompts in both app and worker.
    let root = stable_root()?;
    if journal(&root)?.exists() {
        return Ok(());
    }
    if !root.join("drive-backup.json").exists() {
        return Ok(());
    }
    let c = config(&root)?;
    if !c.enabled || !c.connected {
        return Ok(());
    }
    if !schedule::due(&c, &status(&root)?, chrono::Local::now()) {
        return Ok(());
    }
    let activity = Activity::default();
    activity.set_root(&root)?;
    run(&root, &activity).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locks_coordinate_independent_activities_across_library_replacement() {
        let parent =
            std::env::temp_dir().join(format!("patter-lock-test-{}", uuid::Uuid::new_v4()));
        let root = parent.join("library");
        fs::create_dir_all(&root).unwrap();
        let a = Activity::default();
        a.set_root(&root).unwrap();
        let b = Activity::default();
        b.set_root(&root).unwrap();
        let job = a.job().unwrap();
        assert!(b.exclusive().is_err());
        let second = b.job().unwrap();
        drop(job);
        assert!(a.exclusive().is_err());
        drop(second);
        let exclusive = a.exclusive().unwrap();
        fs::rename(&root, parent.join("safety")).unwrap();
        fs::create_dir_all(&root).unwrap();
        assert!(b.job().is_err());
        drop(exclusive);
        assert!(b.job().is_ok());
        let run = process_lock(&root, "backup", true).unwrap();
        assert!(process_lock(&root, "backup", true).is_err());
        drop(run);
        assert!(process_lock(&root, "backup", true).is_ok());
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn restore_swap_recovers_after_each_rename_without_losing_previous_library() {
        for phase in 0..3 {
            let base = std::env::temp_dir()
                .join(format!("patter-restore-journal-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&base).unwrap();
            let root = base.join("library");
            let target = base.join("restored");
            let safety = base.join("safety");
            fs::create_dir(&root).unwrap();
            fs::write(root.join("data"), b"original").unwrap();
            fs::create_dir(&target).unwrap();
            fs::write(target.join("data"), b"restored").unwrap();
            if phase > 0 {
                fs::rename(&root, &safety).unwrap();
            }
            if phase > 1 {
                fs::rename(&target, &root).unwrap();
            }
            replace_library(&root, &target, &safety).unwrap();
            assert_eq!(fs::read(root.join("data")).unwrap(), b"restored");
            assert_eq!(fs::read(safety.join("data")).unwrap(), b"original");
            fs::remove_dir_all(base).unwrap();
        }
    }
}
