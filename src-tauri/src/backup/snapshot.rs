use super::{atomic_json, hash_file, io, safe_relative};
use crate::store::{self, Result};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    #[serde(default)]
    pub drive_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: u32,
    pub id: String,
    pub created_at: String,
    pub app_version: String,
    pub schema_version: u32,
    pub source_root: String,
    pub device_id: String,
    pub library_id: String,
    pub entries: Vec<Entry>,
}
fn files(base: &Path, path: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    if depth > 40 {
        return Err("Backup folders are nested too deeply.".into());
    }
    for item in fs::read_dir(path).map_err(io)? {
        let p = item.map_err(io)?.path();
        let meta = fs::symlink_metadata(&p).map_err(io)?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "Backup cannot follow a symbolic link: {}",
                p.display()
            ));
        }
        if meta.is_dir() {
            files(base, &p, out, depth + 1)?;
        } else if meta.is_file() {
            out.push(p.strip_prefix(base).map_err(io)?.to_owned());
        } else {
            return Err("A recording folder contains an unsupported special file.".into());
        }
    }
    Ok(())
}
fn entry(base: &Path, relative: &Path) -> Result<Entry> {
    let file = base.join(relative);
    Ok(Entry {
        path: relative.to_string_lossy().into(),
        size: fs::metadata(&file).map_err(io)?.len(),
        sha256: hash_file(&file)?,
        drive_id: String::new(),
    })
}
fn referenced_paths(db: &Connection, root: &Path) -> Result<()> {
    let mut stmt = db
        .prepare("SELECT data FROM meetings UNION ALL SELECT data FROM versions")
        .map_err(io)?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(io)?;
    for raw in rows {
        let m: Value = serde_json::from_str(&raw.map_err(io)?).map_err(io)?;
        for r in m["recordings"].as_array().unwrap_or(&vec![]) {
            let p = Path::new(r["path"].as_str().ok_or("Recording path is missing")?);
            let relative = p
                .strip_prefix(root)
                .map_err(|_| "A recording is outside the library and cannot be backed up.")?;
            safe_relative(relative)?;
            if !relative.starts_with("recordings") || !p.is_file() {
                return Err(format!("A retained recording is missing: {}", p.display()));
            }
        }
    }
    Ok(())
}
pub fn capture(root: &Path, staging: &Path, config: &super::Config) -> Result<Manifest> {
    fs::create_dir(staging).map_err(io)?;
    let db = Connection::open_with_flags(
        root.join("patter.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(io)?;
    let database = staging.join("patter.sqlite3");
    db.backup("main", &database, None).map_err(io)?;
    let frozen =
        Connection::open_with_flags(&database, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(io)?;
    referenced_paths(&frozen, root)?;
    let schema: u32 = frozen
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(io)?;
    let mut manifest = Manifest {
        format: 1,
        id: uuid::Uuid::new_v4().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        schema_version: schema,
        source_root: root.to_string_lossy().into(),
        device_id: config.device_id.clone(),
        library_id: config.library_id.clone(),
        entries: vec![],
    };
    let mut paths = vec![];
    files(root, &root.join("recordings"), &mut paths, 0)?;
    for relative in paths {
        let original = root.join(&relative);
        let digest = hash_file(&original)?;
        let target = staging.join(&relative);
        fs::create_dir_all(target.parent().unwrap()).map_err(io)?;
        let mut src = fs::File::open(&original).map_err(io)?;
        let mut dest = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&target)
            .map_err(io)?;
        std::io::copy(&mut src, &mut dest).map_err(io)?;
        dest.sync_all().map_err(io)?;
        if hash_file(&target)? != digest || hash_file(&original)? != digest {
            return Err("A recording changed during capture. The backup will retry after recording finishes.".into());
        }
        manifest.entries.push(entry(staging, &relative)?);
    }
    let mut config_export = serde_json::to_value(config).map_err(io)?;
    // The account identifier and client identifier are not credentials. Never export Keychain data.
    config_export["enabled"] = json!(false);
    config_export
        .as_object_mut()
        .unwrap()
        .remove("credentialAccount");
    atomic_json(
        &staging.join("configuration.json"),
        &json!({"preferences":store::preferences(&frozen)?,"backup":config_export,"models":serde_json::from_str::<Value>(include_str!("../../transcription-models.json")).map_err(io)?}),
    )?;
    manifest
        .entries
        .push(entry(staging, Path::new("configuration.json"))?);
    for m in store::all(&frozen)? {
        let id = m["id"].as_str().ok_or("Missing conversation ID")?;
        store::valid_id(id)?;
        let prefix = PathBuf::from("conversations")
            .join(id)
            .join(format!("revision-{}", m["revision"].as_i64().unwrap_or(0)));
        fs::create_dir_all(staging.join(&prefix)).map_err(io)?;
        for (name, value) in [
            ("notes.md", m["notes"].as_str().unwrap_or("").to_owned()),
            ("summary.md", m["summary"].as_str().unwrap_or("").to_owned()),
            (
                "transcript.json",
                serde_json::to_string_pretty(&m["transcript"]).map_err(io)?,
            ),
            (
                "conversation.json",
                serde_json::to_string_pretty(&m).map_err(io)?,
            ),
        ] {
            let relative = prefix.join(name);
            fs::write(staging.join(&relative), value).map_err(io)?;
            manifest.entries.push(entry(staging, &relative)?);
        }
    }
    drop(frozen);
    drop(db);
    let mut gz = flate2::write::GzEncoder::new(
        fs::File::create(staging.join("patter.sqlite3.gz")).map_err(io)?,
        flate2::Compression::default(),
    );
    std::io::copy(&mut fs::File::open(&database).map_err(io)?, &mut gz).map_err(io)?;
    gz.finish().map_err(io)?.sync_all().map_err(io)?;
    manifest
        .entries
        .push(entry(staging, Path::new("patter.sqlite3.gz"))?);
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    Ok(manifest)
}
pub fn validate(manifest: &Manifest) -> Result<()> {
    if manifest.format != 1 || manifest.schema_version != 1 {
        return Err("This backup requires a different Patter version.".into());
    }
    let mut seen = HashSet::new();
    for e in &manifest.entries {
        safe_relative(Path::new(&e.path))?;
        if !seen.insert(&e.path)
            || e.sha256.len() != 64
            || !e.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("Invalid or duplicate backup entry.".into());
        }
        if !(e.path.starts_with("recordings/")
            || e.path.starts_with("conversations/")
            || matches!(e.path.as_str(), "patter.sqlite3.gz" | "configuration.json"))
        {
            return Err("Unsupported backup path.".into());
        }
    }
    if !seen.contains(&"patter.sqlite3.gz".to_string())
        || !seen.contains(&"configuration.json".to_string())
    {
        return Err("Backup is incomplete.".into());
    }
    Ok(())
}
fn remap(m: &mut Value, original: &Path, destination: &Path) -> Result<()> {
    if let Some(rows) = m["recordings"].as_array_mut() {
        for row in rows {
            let p = Path::new(row["path"].as_str().ok_or("Missing recording path")?);
            let rel = p
                .strip_prefix(original)
                .map_err(|_| "Recording path is outside the source library")?;
            safe_relative(rel)?;
            if !rel.starts_with("recordings") {
                return Err("Invalid recording path".into());
            }
            row["path"] = json!(destination.join(rel));
        }
    }
    if let Some(p) = m["importSource"]["originalFiles"].as_str() {
        let rel = Path::new(p)
            .strip_prefix(original)
            .map_err(|_| "Invalid imported-file path")?;
        safe_relative(rel)?;
        if !rel.starts_with("recordings") {
            return Err("Invalid imported-file path".into());
        }
        m["importSource"]["originalFiles"] = json!(destination.join(rel));
    }
    Ok(())
}
/// Rebuild data into a fresh schema; immutable-version triggers are never removed from the original.
pub fn restore(
    staging: &Path,
    target: &Path,
    final_root: &Path,
    manifest: &Manifest,
) -> Result<()> {
    validate(manifest)?;
    for e in &manifest.entries {
        let p = staging.join(&e.path);
        if fs::symlink_metadata(&p)
            .map_err(io)?
            .file_type()
            .is_symlink()
            || fs::metadata(&p).map_err(io)?.len() != e.size
            || hash_file(&p)? != e.sha256
        {
            return Err(format!("Backup verification failed for {}", e.path));
        }
    }
    if target.exists() {
        return Err("Restore destination already exists.".into());
    }
    let raw = staging.join(format!("verified-{}.sqlite3", uuid::Uuid::new_v4()));
    let mut decoder = flate2::read::GzDecoder::new(
        fs::File::open(staging.join("patter.sqlite3.gz")).map_err(io)?,
    );
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&raw)
        .map_err(io)?;
    // Reject pathological decompression without imposing a small meeting-library limit.
    let copied = std::io::copy(
        &mut decoder.by_ref().take(16 * 1024 * 1024 * 1024 + 1),
        &mut output,
    )
    .map_err(io)?;
    if copied > 16 * 1024 * 1024 * 1024 {
        return Err("The backup database exceeds the supported restore size.".into());
    }
    output.sync_all().map_err(io)?;
    let source = Connection::open_with_flags(raw, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(io)?;
    let integrity: String = source
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(io)?;
    let schema: u32 = source
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(io)?;
    if integrity != "ok" || schema != 1 {
        return Err("The backup database failed validation.".into());
    }
    let lib = store::open(target)?;
    let mut db = lib.db.lock().map_err(|_| "Restore lock failed")?;
    let tx = db.transaction().map_err(io)?;
    let mut stmt = source
        .prepare("SELECT id,revision,saved_at,data FROM versions ORDER BY id,revision")
        .map_err(io)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(io)?;
    for row in rows {
        let (id, revision, saved, raw) = row.map_err(io)?;
        store::valid_id(&id)?;
        let mut m: Value = serde_json::from_str(&raw).map_err(io)?;
        remap(&mut m, Path::new(&manifest.source_root), final_root)?;
        tx.execute(
            "INSERT INTO versions(id,revision,saved_at,data) VALUES(?,?,?,?)",
            rusqlite::params![id, revision, saved, m.to_string()],
        )
        .map_err(io)?;
    }
    for mut m in store::all(&source)? {
        remap(&mut m, Path::new(&manifest.source_root), final_root)?;
        store::valid_id(m["id"].as_str().ok_or("Missing conversation ID")?)?;
        tx.execute(
            "INSERT INTO meetings(id,data) VALUES(?,?)",
            rusqlite::params![m["id"].as_str().unwrap(), m.to_string()],
        )
        .map_err(io)?;
    }
    tx.execute(
        "INSERT INTO preferences(id,data) VALUES(1,?)",
        [store::preferences(&source)?.to_string()],
    )
    .map_err(io)?;
    tx.commit().map_err(io)?;
    for e in &manifest.entries {
        if e.path.starts_with("recordings/") {
            let dest = target.join(&e.path);
            fs::create_dir_all(dest.parent().unwrap()).map_err(io)?;
            fs::copy(staging.join(&e.path), &dest).map_err(io)?;
            fs::File::open(dest).map_err(io)?.sync_all().map_err(io)?;
        }
    }
    // Verify every historical recording was included, after translating paths to the staging target.
    let mut stmt = db
        .prepare("SELECT data FROM meetings UNION ALL SELECT data FROM versions")
        .map_err(io)?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(io)?;
    for row in rows {
        let m: Value = serde_json::from_str(&row.map_err(io)?).map_err(io)?;
        for recording in m["recordings"].as_array().unwrap_or(&vec![]) {
            let relative = Path::new(recording["path"].as_str().ok_or("Missing audio path")?)
                .strip_prefix(final_root)
                .map_err(io)?;
            if !target.join(relative).is_file() {
                return Err("Backup is missing a historical recording.".into());
            }
        }
    }
    let exported: Value =
        serde_json::from_slice(&fs::read(staging.join("configuration.json")).map_err(io)?)
            .map_err(io)?;
    let mut config: super::Config =
        serde_json::from_value(exported["backup"].clone()).map_err(io)?;
    config.enabled = false;
    config.connected = false;
    config.credential_account.clear();
    config.email.clear();
    config.folder_id.clear();
    config.device_id = uuid::Uuid::new_v4().to_string();
    config.library_id = uuid::Uuid::new_v4().to_string();
    atomic_json(&target.join("drive-backup.json"), &config)?;
    drop(stmt);
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .map_err(io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        base: PathBuf,
        lib: store::Library,
        id: String,
        c: super::super::Config,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
    fn fixture() -> Fixture {
        let base =
            std::env::temp_dir().join(format!("patter-backup-test-{}", uuid::Uuid::new_v4()));
        let lib = store::open(&base.join("old-mac")).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let dir = lib.root.join("recordings").join(&id);
        fs::create_dir_all(dir.join("anarlog-source")).unwrap();
        fs::write(dir.join("audio.wav"), b"retained original audio bytes").unwrap();
        fs::write(
            dir.join("anarlog-source/_memo.md"),
            b"original imported notes",
        )
        .unwrap();
        let mut db = lib.db.lock().unwrap();
        let first=store::save(&mut db,json!({"id":id,"title":"Fixture","notes":"first notes","summary":"first summary","recordings":[{"path":dir.join("audio.wav")}],"transcript":[{"text":"hello"}],"importSource":{"originalFiles":dir.join("anarlog-source")}})).unwrap();
        let mut next = first;
        next["notes"] = json!("latest notes");
        next["archived"] = json!(true);
        store::save(&mut db, next).unwrap();
        db.execute("INSERT INTO preferences(id,data) VALUES(1,?)",[json!({"model":"local-test","summaryTemplate":"interview","templateInstructions":{"interview":"custom instructions"}}).to_string()]).unwrap();
        drop(db);
        Fixture {
            base,
            lib,
            id,
            c: super::super::Config::default(),
        }
    }
    #[test]
    fn restores_audio_history_imports_and_preferences_on_another_mac() {
        let mut f = fixture();
        f.c.credential_account = "local-keychain-item".into();
        let stage = f.base.join("staging");
        let m = capture(&f.lib.root, &stage, &f.c).unwrap();
        let exported: Value =
            serde_json::from_slice(&fs::read(stage.join("configuration.json")).unwrap()).unwrap();
        assert!(exported["backup"].get("credentialAccount").is_none());
        let target = f.base.join("new-mac");
        restore(&stage, &target, &target, &m).unwrap();
        let restored = store::open(&target).unwrap();
        let db = restored.db.lock().unwrap();
        let latest = store::load(&db, &f.id).unwrap();
        assert_eq!(latest["notes"], "latest notes");
        assert_eq!(latest["archived"], true);
        let paths: Vec<String> = db
            .prepare("SELECT data FROM versions ORDER BY revision")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(paths.len(), 2);
        let first: Value = serde_json::from_str(&paths[0]).unwrap();
        assert_eq!(first["notes"], "first notes");
        assert_eq!(first["summary"], "first summary");
        for raw in paths {
            let v: Value = serde_json::from_str(&raw).unwrap();
            let p = Path::new(v["recordings"][0]["path"].as_str().unwrap());
            assert!(p.starts_with(&target));
            assert_eq!(fs::read(p).unwrap(), b"retained original audio bytes");
        }
        assert_eq!(
            fs::read(
                Path::new(latest["importSource"]["originalFiles"].as_str().unwrap())
                    .join("_memo.md")
            )
            .unwrap(),
            b"original imported notes"
        );
        assert_eq!(
            store::preferences(&db).unwrap()["templateInstructions"]["interview"],
            "custom instructions"
        );
        assert!(db.execute("DELETE FROM versions", []).is_err());
        assert!(db.execute("UPDATE versions SET data='{}'", []).is_err());
        let config: super::super::Config =
            super::super::read_json(&target.join("drive-backup.json")).unwrap();
        assert!(!config.connected);
        assert!(config.credential_account.is_empty());
        assert!(!config.enabled);
        assert_ne!(config.library_id, f.c.library_id);
        assert_eq!(
            store::load(&f.lib.db.lock().unwrap(), &f.id).unwrap()["notes"],
            "latest notes"
        );
    }
    #[test]
    fn rejects_corruption_missing_audio_and_unsafe_manifests() {
        let f = fixture();
        let stage = f.base.join("staging");
        let mut m = capture(&f.lib.root, &stage, &f.c).unwrap();
        fs::write(stage.join("configuration.json"), b"corrupted").unwrap();
        let target = f.base.join("rejected");
        assert!(restore(&stage, &target, &target, &m).is_err());
        assert!(!target.exists());
        m.entries[0].path = "../escape".into();
        assert!(validate(&m).is_err());
        fs::remove_file(f.lib.root.join("recordings").join(&f.id).join("audio.wav")).unwrap();
        assert!(capture(&f.lib.root, &f.base.join("missing"), &f.c)
            .unwrap_err()
            .contains("missing"));
    }
    #[test]
    fn unchanged_audio_and_exports_keep_hashes_and_models_are_excluded() {
        let f = fixture();
        fs::create_dir_all(f.lib.root.join("models")).unwrap();
        fs::write(f.lib.root.join("models/weights.bin"), b"do not upload").unwrap();
        let a = capture(&f.lib.root, &f.base.join("a"), &f.c).unwrap();
        let b = capture(&f.lib.root, &f.base.join("b"), &f.c).unwrap();
        for entry in &a.entries {
            assert_eq!(
                entry.sha256,
                b.entries
                    .iter()
                    .find(|e| e.path == entry.path)
                    .unwrap()
                    .sha256
            );
            assert!(!entry.path.starts_with("models/"));
        }
        assert!(a.entries.iter().any(|e| e.path.ends_with("notes.md")));
        assert!(a.entries.iter().any(|e| e.path.ends_with("summary.md")));
    }
    #[test]
    fn symbolic_links_cannot_escape_recordings() {
        let f = fixture();
        std::os::unix::fs::symlink("/tmp", f.lib.root.join("recordings/outside")).unwrap();
        assert!(capture(&f.lib.root, &f.base.join("stage"), &f.c)
            .unwrap_err()
            .contains("symbolic link"));
    }
}
