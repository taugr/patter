use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
pub struct Library {
    pub db: Mutex<Connection>,
    pub root: PathBuf,
}
pub type Result<T> = std::result::Result<T, String>;
pub fn open(root: &Path) -> Result<Library> {
    std::fs::create_dir_all(root.join("recordings")).map_err(|e| e.to_string())?;
    let mut db = Connection::open(root.join("patter.sqlite3")).map_err(|e| e.to_string())?;
    let version: u32 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if version > 1 {
        return Err(format!("This library uses schema {version}. Install a newer Patter; your library has not been changed."));
    }
    db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;")
        .map_err(|e| e.to_string())?;
    if version == 0 {
        migrate(&mut db, root, 1, "
          CREATE TABLE IF NOT EXISTS meetings(id TEXT PRIMARY KEY, data TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS versions(id TEXT NOT NULL, revision INTEGER NOT NULL, saved_at TEXT NOT NULL, data TEXT NOT NULL, PRIMARY KEY(id,revision));
          CREATE TABLE IF NOT EXISTS preferences(id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL);
          CREATE TRIGGER IF NOT EXISTS protect_meetings BEFORE DELETE ON meetings BEGIN SELECT RAISE(ABORT,'Archive conversations instead of deleting them'); END;
          CREATE TRIGGER IF NOT EXISTS protect_versions_delete BEFORE DELETE ON versions BEGIN SELECT RAISE(ABORT,'Versions are permanent'); END;
          CREATE TRIGGER IF NOT EXISTS protect_versions_update BEFORE UPDATE ON versions BEGIN SELECT RAISE(ABORT,'Versions are immutable'); END;
        ")?;
    }
    Ok(Library {
        db: Mutex::new(db),
        root: root.to_owned(),
    })
}
/// SQLite online backup includes WAL commits. Audio is immutable and never changed by migrations.
/// Keep all snapshots; disk-full or metadata failures abort the operation before it changes data.
pub fn snapshot(db: &Connection, root: &Path, reason: &str) -> Result<PathBuf> {
    let destination = root.join("backups").join(format!(
        "{}-{}",
        chrono::Utc::now().format("%Y%m%dT%H%M%S"),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&destination)
        .map_err(|e| format!("Backup could not be created: {e}"))?;
    db.backup("main", destination.join("patter.sqlite3"), None)
        .map_err(|e| format!("Library backup failed: {e}"))?;
    let version: u32 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(|e| e.to_string())?;
    std::fs::write(destination.join("manifest.json"), json!({"appVersion":env!("CARGO_PKG_VERSION"),"schemaVersion":version,"reason":reason,"createdAt":chrono::Utc::now().to_rfc3339(),"recordings":"Original audio remains in the library recordings directory; this snapshot backs up the database only."}).to_string()).map_err(|e| e.to_string())?;
    Ok(destination)
}
fn migrate(db: &mut Connection, root: &Path, target: u32, sql: &str) -> Result<()> {
    snapshot(db, root, &format!("before-schema-{target}"))?;
    let tx = db.transaction().map_err(|e| e.to_string())?;
    tx.execute_batch(sql).map_err(|e| e.to_string())?;
    tx.pragma_update(None, "user_version", target)
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}
pub fn valid_id(id: &str) -> Result<()> {
    if uuid::Uuid::parse_str(id).is_ok() {
        Ok(())
    } else {
        Err("Invalid conversation identifier".into())
    }
}
pub fn load(db: &Connection, id: &str) -> Result<Value> {
    let text: String = db
        .query_row("SELECT data FROM meetings WHERE id=?", [id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}
pub fn save(db: &mut Connection, mut data: Value) -> Result<Value> {
    let id = data["id"]
        .as_str()
        .ok_or("Missing conversation id")?
        .to_owned();
    valid_id(&id)?;
    if !data["title"].is_string() || !data["notes"].is_string() || !data["recordings"].is_array() {
        return Err("Invalid conversation data".into());
    }
    let tx = db.transaction().map_err(|e| e.to_string())?;
    let revision: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(revision),0)+1 FROM versions WHERE id=?",
            [&id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    // A UI save may not detach any existing recordings, even when restoring a version.
    if let Ok(old) = load(&tx, &id) {
        if let Some(old_recordings) = old["recordings"].as_array() {
            let incoming = data["recordings"].as_array_mut().unwrap();
            for recording in old_recordings {
                incoming.retain(|r| r["id"] != recording["id"]);
                incoming.push(recording.clone());
            }
        }
    }
    data["revision"] = json!(revision);
    let text = serde_json::to_string(&data).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO versions(id,revision,saved_at,data) VALUES(?,?,?,?)",
        params![id, revision, chrono::Utc::now().to_rfc3339(), text],
    )
    .map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO meetings(id,data) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET data=excluded.data", params![id, text]).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(data)
}
pub fn all(db: &Connection) -> Result<Vec<Value>> {
    let mut s = db
        .prepare("SELECT data FROM meetings ORDER BY json_extract(data,'$.createdAt') DESC")
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string()))
        .collect()
}
pub fn preferences(db: &Connection) -> Result<Value> {
    let raw = db.query_row("SELECT data FROM preferences WHERE id=1", [], |r| {
        r.get::<_, String>(0)
    });
    match raw {
        Ok(s) => serde_json::from_str(&s).map_err(|e| e.to_string()),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(
            json!({"endpoint":"http://127.0.0.1:1234/v1","model":"","whisperModel":"","calendarEnabled":false}),
        ),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        json!({"id":uuid::Uuid::new_v4().to_string(),"title":"A conversation","notes":"original","recordings":[],"archived":false})
    }
    fn memory() -> Library {
        let path = std::env::temp_dir().join(format!("patter-test-{}", uuid::Uuid::new_v4()));
        open(&path).unwrap()
    }
    #[test]
    fn keeps_versions_and_prevents_deletion() {
        let lib = memory();
        let mut db = lib.db.lock().unwrap();
        let first = save(&mut db, fixture()).unwrap();
        let mut next = first.clone();
        next["notes"] = json!("edited");
        next["archived"] = json!(true);
        save(&mut db, next).unwrap();
        let n: i64 = db
            .query_row("SELECT COUNT(*) FROM versions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
        assert!(db.execute("DELETE FROM meetings", []).is_err());
        assert!(db.execute("DELETE FROM versions", []).is_err());
        assert!(db.execute("UPDATE versions SET data='{}'", []).is_err());
    }
    #[test]
    fn summary_regeneration_keeps_the_previous_text_and_template_snapshot() {
        let lib = memory();
        let mut db = lib.db.lock().unwrap();
        let mut original = fixture();
        original["summary"] = json!("Original overview");
        original["summarySource"] =
            json!({"templateId":"general","instructions":"Original instructions"});
        let first = save(&mut db, original).unwrap();
        let mut regenerated = first.clone();
        regenerated["summary"] = json!("Interview overview");
        regenerated["summaryTemplate"] = json!("interview");
        regenerated["summarySource"] =
            json!({"templateId":"interview","instructions":"Revised instructions"});
        let latest = save(&mut db, regenerated).unwrap();
        let retained: String = db
            .query_row(
                "SELECT data FROM versions WHERE id=? AND revision=1",
                [first["id"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(serde_json::from_str::<Value>(&retained).unwrap(), first);
        assert_eq!(load(&db, first["id"].as_str().unwrap()).unwrap(), latest);
    }
    #[test]
    fn restoring_never_detaches_audio() {
        let lib = memory();
        let mut db = lib.db.lock().unwrap();
        let first = save(&mut db, fixture()).unwrap();
        let mut recorded = first.clone();
        recorded["recordings"] = json!([{"id":"audio-a","path":"original.wav"}]);
        save(&mut db, recorded).unwrap();
        let restored = save(&mut db, first).unwrap();
        assert_eq!(restored["recordings"].as_array().unwrap().len(), 1);
        assert_eq!(restored["revision"], 3);
    }
    #[test]
    fn rejects_path_ids() {
        assert!(valid_id("../../other").is_err());
    }
    #[test]
    fn newer_schema_is_rejected_without_resetting_it() {
        let lib = memory();
        lib.db
            .lock()
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        assert!(open(&lib.root).is_err());
        let version: u32 = lib
            .db
            .lock()
            .unwrap()
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 99);
    }
    #[test]
    fn failed_migration_rolls_back_and_backup_restores_versions() {
        let lib = memory();
        let mut db = lib.db.lock().unwrap();
        let original = save(&mut db, fixture()).unwrap();
        let backup = snapshot(&db, &lib.root, "test-update").unwrap();
        assert!(migrate(
            &mut db,
            &lib.root,
            2,
            "CREATE TABLE transient(id INTEGER); INVALID SQL;"
        )
        .is_err());
        let version: u32 = db
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version, 1);
        assert!(db.prepare("SELECT * FROM transient").is_err());
        let restored = Connection::open(backup.join("patter.sqlite3")).unwrap();
        assert_eq!(all(&restored).unwrap(), vec![original]);
        let versions: u32 = restored
            .query_row("SELECT COUNT(*) FROM versions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(versions, 1);
    }
    #[test]
    fn failed_backup_aborts_migration() {
        let lib = memory();
        let mut db = lib.db.lock().unwrap();
        let blocked = lib.root.join("blocked");
        std::fs::write(&blocked, b"not a directory").unwrap();
        assert!(migrate(
            &mut db,
            &blocked,
            2,
            "CREATE TABLE should_not_exist(id INTEGER);"
        )
        .is_err());
        assert!(db.prepare("SELECT * FROM should_not_exist").is_err());
    }
}
