//! Read-only import of the desktop_v1.0.27 session-folder format.
//! The original files travel with each imported conversation, including attachments.
use crate::store::{self, Result};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub fingerprint: String,
    pub sessions: Vec<SessionPreview>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPreview {
    pub title: String,
    pub recordings: usize,
    pub already_imported: bool,
}
struct File {
    relative: PathBuf,
    hash: String,
}
struct Session {
    source: PathBuf,
    key: String,
    meeting: Value,
    files: Vec<File>,
}
struct Plan {
    preview: Preview,
    sessions: Vec<Session>,
}

fn io(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn hash(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(io)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(io)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn entries(path: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(path)
        .map_err(io)?
        .map(|e| e.map(|e| e.path()).map_err(io))
        .collect::<Result<Vec<_>>>()?;
    paths.sort();
    Ok(paths)
}
fn regular(path: &Path) -> Result<fs::Metadata> {
    let meta = fs::symlink_metadata(path).map_err(io)?;
    if meta.file_type().is_symlink() || !(meta.is_file() || meta.is_dir()) {
        return Err(format!(
            "Unsupported link or special file: {}. Copy the actual files into the folder first.",
            path.display()
        ));
    }
    Ok(meta)
}
fn discover(path: &Path, depth: usize, out: &mut Vec<PathBuf>) -> Result<()> {
    if depth > 32 {
        return Err("The source folder is nested too deeply.".into());
    }
    regular(path)?;
    if path.join("_meta.json").exists() {
        out.push(path.to_owned());
        return Ok(());
    }
    for entry in entries(path)? {
        if entry
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        {
            continue;
        }
        if regular(&entry)?.is_dir() {
            discover(&entry, depth + 1, out)?;
        }
    }
    Ok(())
}
fn inventory(base: &Path, path: &Path, depth: usize, out: &mut Vec<File>) -> Result<()> {
    if depth > 32 {
        return Err("The session folder is nested too deeply.".into());
    }
    for entry in entries(path)? {
        let meta = regular(&entry)?;
        if meta.is_dir() {
            inventory(base, &entry, depth + 1, out)?;
        } else {
            out.push(File {
                relative: entry.strip_prefix(base).map_err(io)?.to_owned(),
                hash: hash(&entry)?,
            });
        }
    }
    Ok(())
}
fn text(path: &Path) -> Result<String> {
    if regular(path)?.len() > 100 * 1024 * 1024 {
        return Err(format!("Text file is too large: {}", path.display()));
    }
    fs::read_to_string(path).map_err(io)
}
fn markdown(raw: &str) -> String {
    let normalized = raw.replace("\r\n", "\n");
    if let Some(rest) = normalized.strip_prefix("---\n") {
        if let Some((_, body)) = rest.split_once("\n---\n") {
            return body.trim().to_owned();
        }
    }
    normalized.trim().to_owned()
}
fn timestamp(v: &Value) -> Option<String> {
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.to_rfc3339())
}
fn number(v: &Value) -> f64 {
    v.as_f64().filter(|n| n.is_finite()).unwrap_or(0.0)
}
fn transcripts(value: &Value, session_id: &str) -> Result<(Vec<Value>, f64)> {
    let rows = value["transcripts"]
        .as_array()
        .ok_or("Expected a transcripts array")?;
    let base = rows
        .iter()
        .filter_map(|r| r["started_at"].as_f64())
        .fold(f64::INFINITY, f64::min);
    let mut result: Vec<Value> = vec![];
    let mut duration: f64 = 0.0;
    for row in rows {
        if row["session_id"]
            .as_str()
            .is_some_and(|id| id != session_id)
        {
            return Err("Transcript belongs to a different session".into());
        }
        let offset = if base.is_finite() {
            (row["started_at"].as_f64().unwrap_or(base) - base).max(0.0) / 1000.0
        } else {
            0.0
        };
        let mut phrase: Option<Value> = None;
        let empty = vec![];
        let words = if row["words"].is_null() {
            &empty
        } else {
            row["words"].as_array().ok_or("Invalid transcript words")?
        };
        for word in words {
            let content = word["text"]
                .as_str()
                .ok_or("A transcript word has no text")?;
            let start = offset + number(&word["start_ms"]).max(0.0) / 1000.0;
            duration = duration.max(offset + number(&word["end_ms"]).max(0.0) / 1000.0);
            let speaker = word["speaker"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    if number(&word["channel"]) == 0.0 {
                        "Microphone".into()
                    } else {
                        "Computer audio".into()
                    }
                });
            if phrase
                .as_ref()
                .is_some_and(|p| p["speaker"] != speaker || start - number(&p["start"]) >= 12.0)
            {
                result.push(phrase.take().unwrap());
            }
            if let Some(p) = phrase.as_mut() {
                let old = p["text"].as_str().unwrap();
                let separator = if old.ends_with(char::is_whitespace)
                    || content.starts_with(char::is_whitespace)
                    || content.starts_with(['.', ',', '!', '?', ';', ':'])
                {
                    ""
                } else {
                    " "
                };
                p["text"] = json!(format!("{old}{separator}{content}"));
            } else {
                phrase = Some(json!({"start":start,"text":content,"speaker":speaker}));
            }
        }
        if let Some(p) = phrase {
            result.push(p);
        }
        if words.is_empty() && row["memo_md"].as_str().is_some_and(|s| !s.is_empty()) {
            result.push(
                json!({"start":offset,"text":row["memo_md"],"speaker":"Imported transcript"}),
            );
        }
    }
    result.sort_by(|a, b| number(&a["start"]).total_cmp(&number(&b["start"])));
    Ok((result, duration))
}
fn session(path: &Path, warnings: &mut Vec<String>) -> Result<Session> {
    let meta: Value = serde_json::from_str(&text(&path.join("_meta.json"))?).map_err(io)?;
    let source_id = meta["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Session metadata has no id")?;
    let title = meta["title"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("Untitled Anarlog conversation");
    let mut files = vec![];
    inventory(path, path, 0, &mut files)?;
    let mut digest = Sha256::new();
    digest.update(source_id.as_bytes());
    for f in &files {
        digest.update([0]);
        digest.update(f.relative.to_string_lossy().as_bytes());
        digest.update([0]);
        digest.update(f.hash.as_bytes());
    }
    let key = format!("{:x}", digest.finalize());
    let mut summaries = vec![];
    let mut notes = String::new();
    let mut recordings = vec![];
    for f in &files {
        if f.relative.components().count() != 1 {
            continue;
        }
        let name = f.relative.to_string_lossy();
        if name.ends_with(".md") {
            let body = markdown(&text(&path.join(&f.relative))?);
            if name == "_memo.md" {
                notes = body;
            } else if !body.is_empty() {
                summaries.push(format!("## {}\n\n{}", name.trim_end_matches(".md"), body));
            }
        }
        if matches!(name.as_ref(), "audio.mp3" | "audio.wav" | "audio.ogg") {
            recordings.push(json!({"id":uuid::Uuid::new_v4().to_string(),"name":name,"track":"Anarlog recording","offset":0,"sourceFile":name}));
        }
    }
    let transcript_path = path.join("transcript.json");
    let (transcript, duration) = if transcript_path.exists() {
        transcripts(
            &serde_json::from_str::<Value>(&text(&transcript_path)?).map_err(io)?,
            source_id,
        )?
    } else {
        (vec![], 0.0)
    };
    if recordings.is_empty() {
        warnings.push(format!(
            "{title}: no retained audio found. Notes and transcripts can still be imported."
        ));
    }
    let created = timestamp(&meta["created_at"]).or_else(|| timestamp(&meta["createdAt"]));
    if created.is_none() {
        warnings.push(format!(
            "{title}: no valid creation date; the import date will be used."
        ));
    }
    Ok(Session {
        source: path.to_owned(),
        key: key.clone(),
        files,
        meeting: json!({
            "id":uuid::Uuid::new_v4().to_string(),"title":title,"createdAt":created.unwrap_or_else(||chrono::Utc::now().to_rfc3339()),
            "duration":duration,"notes":notes,"summary":summaries.join("\n\n"),"transcript":transcript,"recordings":recordings,
            "decisions":[],"actions":[],"archived":false,"revision":0,
            "importSource":{"app":"anarlog","version":"1.0.27","sessionId":source_id,"fingerprint":key,"metadata":meta}
        }),
    })
}
fn plan(source: &Path, db: &Connection) -> Result<Plan> {
    let root = source.canonicalize().map_err(io)?;
    let sessions_root = if root.join("sessions").is_dir() {
        root.join("sessions")
    } else {
        root
    };
    let mut paths = vec![];
    discover(&sessions_root, 0, &mut paths)?;
    if paths.is_empty() {
        return Err("No Anarlog 1.0.27 sessions found. Choose its Content folder or the sessions folder containing _meta.json files.".into());
    }
    let mut sessions = vec![];
    let mut warnings = vec![];
    for path in paths {
        sessions.push(session(&path, &mut warnings).map_err(|e| {
            format!(
                "Could not read {}: {e}. No conversations were imported.",
                path.display()
            )
        })?);
    }
    let existing: HashSet<String> = store::all(db)?
        .iter()
        .filter(|m| m["importSource"]["app"] == "anarlog")
        .filter_map(|m| m["importSource"]["fingerprint"].as_str().map(str::to_owned))
        .collect();
    let mut seen = existing;
    let mut digest = Sha256::new();
    let previews = sessions
        .iter()
        .map(|s| {
            digest.update(s.key.as_bytes());
            SessionPreview {
                title: s.meeting["title"].as_str().unwrap().into(),
                recordings: s.meeting["recordings"].as_array().unwrap().len(),
                already_imported: !seen.insert(s.key.clone()),
            }
        })
        .collect();
    Ok(Plan {
        preview: Preview {
            fingerprint: format!("{:x}", digest.finalize()),
            sessions: previews,
            warnings,
        },
        sessions,
    })
}
pub fn preview(source: &Path, db: &Connection) -> Result<Preview> {
    Ok(plan(source, db)?.preview)
}

pub fn import(source: &Path, root: &Path, db: &mut Connection, expected: &str) -> Result<Value> {
    let plan = plan(source, db)?;
    if plan.preview.fingerprint != expected {
        return Err(
            "The source changed after the preview. Choose the folder again before importing."
                .into(),
        );
    }
    let pending = plan
        .sessions
        .into_iter()
        .zip(&plan.preview.sessions)
        .filter(|(_, p)| !p.already_imported)
        .map(|(s, _)| s)
        .collect::<Vec<_>>();
    let skipped = plan.preview.sessions.len() - pending.len();
    if pending.is_empty() {
        return Ok(json!({"imported":0,"skipped":skipped}));
    }
    store::snapshot(db, root, "before-anarlog-import")?;
    let mut imported = vec![];
    for mut s in pending {
        let destination = root
            .join("recordings")
            .join(s.meeting["id"].as_str().unwrap())
            .join("anarlog-source");
        fs::create_dir_all(&destination).map_err(io)?;
        for file in &s.files {
            let input = s.source.join(&file.relative);
            regular(&input)?;
            let output = destination.join(&file.relative);
            fs::create_dir_all(output.parent().unwrap()).map_err(io)?;
            let mut reader = fs::File::open(input).map_err(io)?;
            let mut writer = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)
                .map_err(io)?;
            std::io::copy(&mut reader, &mut writer).map_err(io)?;
            writer.flush().map_err(io)?;
            writer.sync_all().map_err(io)?;
            if hash(&output)? != file.hash {
                return Err("A source file changed while copying. No conversations were imported; retry after closing Anarlog.".into());
            }
        }
        for recording in s.meeting["recordings"].as_array_mut().unwrap() {
            recording["path"] = json!(destination.join(recording["sourceFile"].as_str().unwrap()));
            recording.as_object_mut().unwrap().remove("sourceFile");
        }
        s.meeting["importSource"]["originalFiles"] = json!(destination);
        imported.push(s.meeting);
    }
    // One transaction: a copy or database failure never exposes a partially imported batch.
    let tx = db.transaction().map_err(io)?;
    for meeting in &mut imported {
        meeting["revision"] = json!(1);
        let id = meeting["id"].as_str().unwrap();
        let data = meeting.to_string();
        tx.execute(
            "INSERT INTO meetings(id,data) VALUES(?,?)",
            rusqlite::params![id, data],
        )
        .map_err(io)?;
        tx.execute(
            "INSERT INTO versions(id,revision,saved_at,data) VALUES(?,1,?,?)",
            rusqlite::params![id, chrono::Utc::now().to_rfc3339(), data],
        )
        .map_err(io)?;
    }
    tx.commit().map_err(io)?;
    Ok(json!({"imported":imported.len(),"skipped":skipped}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, store::Library) {
        let base =
            std::env::temp_dir().join(format!("patter-anarlog-test-{}", uuid::Uuid::new_v4()));
        let source = base.join("source");
        let session = source.join("sessions/Work/legacy-1");
        fs::create_dir_all(session.join("attachments")).unwrap();
        fs::write(session.join("_meta.json"), json!({"id":"legacy-1","title":"Planning","created_at":"2025-01-02T10:00:00Z","tags":["work"]}).to_string()).unwrap();
        fs::write(
            session.join("_memo.md"),
            "---\nid: memo\nsession_id: legacy-1\n---\nMy original notes",
        )
        .unwrap();
        fs::write(
            session.join("summary.md"),
            "---\nid: summary\nsession_id: legacy-1\n---\nKeep the launch small.",
        )
        .unwrap();
        fs::write(session.join("audio.wav"), b"original audio bytes").unwrap();
        fs::write(
            session.join("attachments/outline.txt"),
            "Original attachment",
        )
        .unwrap();
        fs::write(session.join("transcript.json"), json!({"transcripts":[
            {"id":"t2","session_id":"legacy-1","started_at":1735812010000.0,"words":[{"text":"Second turn.","start_ms":2000,"end_ms":4000,"channel":1,"speaker":"Pat"}]},
            {"id":"t1","session_id":"legacy-1","started_at":1735812000000.0,"words":[{"text":"Hello","start_ms":1000,"end_ms":1500,"channel":0},{"text":"there.","start_ms":1600,"end_ms":2000,"channel":0}]}
        ]}).to_string()).unwrap();
        (source, store::open(&base.join("destination")).unwrap())
    }
    #[test]
    fn imports_versions_audio_notes_and_originals_without_changing_source() {
        let (source, lib) = fixture();
        let mut db = lib.db.lock().unwrap();
        let before = preview(&source, &db).unwrap();
        assert_eq!(before.sessions.len(), 1);
        assert_eq!(before.sessions[0].recordings, 1);
        let result = import(&source, &lib.root, &mut db, &before.fingerprint).unwrap();
        assert_eq!(result["imported"], 1);
        let mut meeting = store::all(&db).unwrap().remove(0);
        assert_eq!(meeting["notes"], "My original notes");
        assert!(meeting["summary"]
            .as_str()
            .unwrap()
            .contains("Keep the launch small."));
        assert_eq!(meeting["createdAt"], "2025-01-02T10:00:00+00:00");
        assert_eq!(meeting["transcript"][0]["text"], "Hello there.");
        assert_eq!(meeting["transcript"][0]["start"], 1.0);
        assert_eq!(meeting["transcript"][1]["start"], 12.0);
        assert_eq!(meeting["transcript"][1]["speaker"], "Pat");
        assert_eq!(meeting["duration"], 14.0);
        let audio = Path::new(meeting["recordings"][0]["path"].as_str().unwrap());
        assert!(audio.starts_with(&lib.root));
        assert_eq!(fs::read(audio).unwrap(), b"original audio bytes");
        let archive = Path::new(meeting["importSource"]["originalFiles"].as_str().unwrap());
        assert!(archive.join("attachments/outline.txt").exists());
        assert_eq!(
            preview(&source, &db).unwrap().fingerprint,
            before.fingerprint
        );
        meeting["notes"] = json!("Edited in Patter");
        store::save(&mut db, meeting).unwrap();
        // The source location is not part of duplicate identity.
        let moved = source.with_file_name("moved-copy");
        fs::rename(&source, &moved).unwrap();
        let again = import(&moved, &lib.root, &mut db, &before.fingerprint).unwrap();
        assert_eq!(again["skipped"], 1);
        assert_eq!(store::all(&db).unwrap()[0]["notes"], "Edited in Patter");
        fs::write(
            moved.join("sessions/Work/legacy-1/_memo.md"),
            "New source notes",
        )
        .unwrap();
        assert!(import(&moved, &lib.root, &mut db, &before.fingerprint).is_err());
        let changed = preview(&moved, &db).unwrap();
        import(&moved, &lib.root, &mut db, &changed.fingerprint).unwrap();
        assert_eq!(store::all(&db).unwrap().len(), 2);
    }
    #[test]
    fn missing_audio_is_reported_and_bad_transcripts_do_not_silently_disappear() {
        let (source, lib) = fixture();
        let db = lib.db.lock().unwrap();
        let session = source.join("sessions/Work/legacy-1");
        // Test fixture only.
        fs::remove_file(session.join("audio.wav")).unwrap();
        let p = preview(&source, &db).unwrap();
        assert_eq!(p.sessions[0].recordings, 0);
        assert!(p.warnings.iter().any(|w| w.contains("no retained audio")));
        fs::write(session.join("transcript.json"), "{broken").unwrap();
        assert!(preview(&source, &db).is_err());
        assert!(store::all(&db).unwrap().is_empty());
    }
    #[test]
    fn rejects_links_and_unrecognized_sources() {
        let (source, lib) = fixture();
        let db = lib.db.lock().unwrap();
        assert!(preview(&lib.root, &db).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                "/etc/hosts",
                source.join("sessions/Work/legacy-1/attachments/outside.txt"),
            )
            .unwrap();
            assert!(preview(&source, &db)
                .unwrap_err()
                .contains("link or special file"));
        }
    }
    #[test]
    fn database_failure_rolls_back_the_entire_import() {
        let (source, lib) = fixture();
        let mut db = lib.db.lock().unwrap();
        let p = preview(&source, &db).unwrap();
        db.execute_batch("CREATE TRIGGER fail_import BEFORE INSERT ON versions BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
        assert!(import(&source, &lib.root, &mut db, &p.fingerprint).is_err());
        assert!(store::all(&db).unwrap().is_empty());
        assert_eq!(preview(&source, &db).unwrap().fingerprint, p.fingerprint);
        db.execute_batch("DROP TRIGGER fail_import").unwrap();
        assert_eq!(
            import(&source, &lib.root, &mut db, &p.fingerprint).unwrap()["imported"],
            1
        );
    }
}
