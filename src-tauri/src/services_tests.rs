use super::*;

fn library() -> Library {
    store::open(&std::env::temp_dir().join(format!("patter-freshness-{}", uuid::Uuid::new_v4())))
        .unwrap()
}
fn fixture(lib: &Library) -> Value {
    store::save_checked(
        &mut lib.db.lock().unwrap(),
        json!({"id":uuid::Uuid::new_v4().to_string(),"revision":0,"title":"Synthetic review",
            "notes":"The review is Friday.","transcript":[{"start":0,"speaker":"A","text":"Review Friday."}],
            "recordings":[],"summary":"","decisions":[],"actions":[],"archived":false}),
    )
    .unwrap()
}
fn generated(day: &str) -> Value {
    json!({"summary":format!("Review {day}."),"decisions":[format!("Review {day}.")],
        "actions":[format!("Prepare for {day}.")],"summarySource":{"templateId":"general","model":"injected",
        "sourceRevision":999,"sourceConversationId":"untrusted","inputProjectionVersion":999}})
}
fn summarize(lib: &Library, meeting: &Value, day: &str) -> Value {
    let job = prepare(lib, meeting["id"].as_str().unwrap(), "summary", None).unwrap();
    finish(lib, &job, generated(day), None).unwrap()
}
fn save(lib: &Library, meeting: Value) -> Value {
    store::save_checked(&mut lib.db.lock().unwrap(), meeting).unwrap()
}
fn load(lib: &Library, meeting: &Value) -> Value {
    store::load(&lib.db.lock().unwrap(), meeting["id"].as_str().unwrap()).unwrap()
}
fn derived(meeting: &Value) -> Value {
    json!({"summary":meeting["summary"],"decisions":meeting["decisions"],
        "actions":meeting["actions"],"summarySource":meeting["summarySource"]})
}

#[test]
fn notes_and_manual_transcript_changes_preserve_generated_content_across_reopen() {
    let lib = library();
    let original = fixture(&lib);
    let mut summary = summarize(&lib, &original, "Friday");
    assert_eq!(summary["summaryFreshness"], "current");
    assert_eq!(
        summary["summarySource"]["sourceRevision"],
        original["revision"]
    );
    assert_eq!(
        summary["summarySource"]["sourceConversationId"],
        original["id"]
    );
    summary["actions"][0]["done"] = json!(true);
    let completed = save(&lib, summary);
    let mut notes = completed.clone();
    notes["notes"] = json!("The review is Monday.");
    // Incoming derived fields cannot forge freshness or its source input.
    notes["summaryFreshness"] = json!("current");
    notes["summaryInput"]["text"] = json!("forged");
    let notes = save(&lib, notes);
    assert_eq!(notes["summaryFreshness"], "changed");
    assert_eq!(derived(&notes), derived(&completed));
    let job = prepare(&lib, original["id"].as_str().unwrap(), "transcript", None).unwrap();
    let transcript = finish(
        &lib,
        &job,
        json!([{"start":0,"text":"Review Monday."}]),
        None,
    )
    .unwrap();
    assert_eq!(transcript["summaryFreshness"], "changed");
    assert_eq!(derived(&transcript), derived(&completed));
    let reopened = store::open(&lib.root).unwrap();
    assert_eq!(load(&reopened, &original), transcript);
    assert_eq!(
        store::all(&reopened.db.lock().unwrap()).unwrap(),
        vec![transcript]
    );
    let db = reopened.db.lock().unwrap();
    let raw: String = db
        .query_row("SELECT data FROM meetings", [], |r| r.get(0))
        .unwrap();
    assert!(!raw.contains("summaryInput"));
    assert!(!raw.contains("summaryFreshness"));
    let version: u32 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 1); // Existing JSON schema requires no migration.
}

#[test]
fn successful_regeneration_captures_new_input_but_failure_keeps_old_state() {
    let lib = library();
    let original = fixture(&lib);
    let friday = summarize(&lib, &original, "Friday");
    let mut monday = friday;
    monday["notes"] = json!("The review is Monday.");
    monday["transcript"][0]["text"] = json!("Review Monday.");
    monday["actions"][0]["done"] = json!(true);
    let monday = save(&lib, monday);
    let job = prepare(&lib, original["id"].as_str().unwrap(), "summary", None).unwrap();
    let mut failed = generated("Monday");
    failed["actions"] = json!("invalid output");
    assert!(finish(&lib, &job, failed, None).is_err());
    assert_eq!(load(&lib, &original), monday);
    assert_eq!(monday["summaryFreshness"], "changed");
    let regenerated = finish(&lib, &job, generated("Monday"), None).unwrap();
    assert_eq!(regenerated["summaryFreshness"], "current");
    assert_eq!(
        regenerated["summarySource"]["sourceRevision"],
        monday["revision"]
    );
    assert_eq!(
        regenerated["summaryInput"]["text"],
        crate::summary_input::project(&job.meeting)
    );
    assert_eq!(regenerated["summary"], "Review Monday.");
    // Existing explicit regeneration replaces actions; historical completion remains intact.
    let raw: String = lib
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT data FROM versions WHERE id=? AND revision=?",
            rusqlite::params![
                monday["id"].as_str().unwrap(),
                monday["revision"].as_i64().unwrap()
            ],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        serde_json::from_str::<Value>(&raw).unwrap()["actions"][0]["done"]
            .as_bool()
            .unwrap()
    );
}

#[test]
fn identical_transcripts_unrelated_edits_and_exact_undo_match_the_original_input() {
    let lib = library();
    let original = fixture(&lib);
    let summary = summarize(&lib, &original, "Friday");
    let job = prepare(&lib, original["id"].as_str().unwrap(), "transcript", None).unwrap();
    let same = finish(&lib, &job, original["transcript"].clone(), None).unwrap();
    assert!(same["revision"].as_i64() > summary["revision"].as_i64());
    assert_eq!(same["summaryFreshness"], "current");
    let mut unrelated = same;
    unrelated["actions"][0]["done"] = json!(true);
    unrelated["archived"] = json!(true);
    unrelated["transcript"][0]["start"] = json!(50);
    unrelated["transcript"][0]["speaker"] = json!("B");
    unrelated["summaryTemplate"] = json!("interview");
    unrelated["summaryInstructions"] = json!("New configuration");
    let unrelated = save(&lib, unrelated);
    assert_eq!(unrelated["summaryFreshness"], "current");
    assert_eq!(unrelated["summarySource"], summary["summarySource"]);
    let mut title = unrelated;
    title["title"] = json!("Updated title");
    let mut title = save(&lib, title);
    assert_eq!(title["summaryFreshness"], "changed");
    title["title"] = original["title"].clone();
    assert_eq!(save(&lib, title)["summaryFreshness"], "current");
}

#[test]
fn both_processing_jobs_keep_revision_conflicts_and_the_newer_state() {
    for kind in ["summary", "transcript"] {
        let lib = library();
        let original = fixture(&lib);
        let summary = summarize(&lib, &original, "Friday");
        let job = prepare(&lib, original["id"].as_str().unwrap(), kind, None).unwrap();
        let mut edit = summary;
        edit["notes"] = json!("New concurrent notes.");
        let saved = save(&lib, edit);
        let result = if kind == "summary" {
            generated("Monday")
        } else {
            json!([{"start":0,"text":"Obsolete"}])
        };
        assert!(finish(&lib, &job, result, None)
            .unwrap_err()
            .contains("REVISION_CONFLICT"));
        assert_eq!(load(&lib, &original), saved);
        let count: i64 = lib
            .db
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM versions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }
}

#[test]
fn legacy_and_unresolvable_provenance_are_unknown_without_migrating_history() {
    let lib = library();
    let mut legacy = fixture(&lib);
    legacy["summary"] = json!("An older summary.");
    legacy["summarySource"] = json!({"model":"old","generatedAt":"2020-01-01T00:00:00Z"});
    let legacy = save(&lib, legacy);
    assert_eq!(legacy["summaryFreshness"], "unknown");
    assert!(legacy.get("summaryInput").is_none());
    assert_eq!(load(&store::open(&lib.root).unwrap(), &legacy), legacy);
    let mut missing = legacy.clone();
    missing["summarySource"]["sourceConversationId"] = legacy["id"].clone();
    missing["summarySource"]["sourceRevision"] = json!(999);
    missing["summarySource"]["inputProjectionVersion"] = json!(1);
    assert_eq!(save(&lib, missing)["summaryFreshness"], "unknown");
    let mut unsupported = load(&lib, &legacy);
    unsupported["summarySource"]["sourceRevision"] = json!(1);
    unsupported["summarySource"]["inputProjectionVersion"] = json!(2);
    assert_eq!(save(&lib, unsupported)["summaryFreshness"], "unknown");
    let unavailable = rusqlite::Connection::open_in_memory().unwrap();
    assert_eq!(
        crate::summary_input::annotate(&unavailable, legacy)["summaryFreshness"],
        "unknown"
    );
}

#[test]
fn copies_and_restores_resolve_the_original_source_conversation() {
    let lib = library();
    let original = fixture(&lib);
    let summary = summarize(&lib, &original, "Friday");
    let mut copy = summary.clone();
    copy["id"] = json!(uuid::Uuid::new_v4().to_string());
    copy["revision"] = json!(0);
    copy["title"] = json!("A draft copy");
    let copied = save(&lib, copy);
    assert_eq!(copied["summaryFreshness"], "changed");
    assert_eq!(
        copied["summarySource"]["sourceConversationId"],
        original["id"]
    );
    assert_eq!(copied["summaryInput"], summary["summaryInput"]);
    let mut edited = summary.clone();
    edited["notes"] = json!("Monday");
    let edited = save(&lib, edited);
    let mut restore = summary;
    restore["revision"] = edited["revision"].clone();
    let restored = save(&lib, restore);
    assert_eq!(restored["summaryFreshness"], "current");
    assert_eq!(
        restored["summarySource"]["sourceRevision"],
        original["revision"]
    );
}

#[test]
fn unified_generation_preserves_legacy_configuration_records_and_historical_exports() {
    let lib = library();
    let fresh = store::preferences(&lib.db.lock().unwrap()).unwrap();
    assert_eq!(fresh["model"], "");
    assert!(fresh["summaryTemplate"].is_null());
    let mut old = fixture(&lib);
    old["summary"] = json!("Original custom summary");
    old["summaryTemplate"] = json!("no-longer-known");
    old["summaryInstructions"] = json!("x".repeat(5000));
    old["summarySource"] = json!({"templateId":"custom","instructions":"Historic custom guidance"});
    old["actions"] = json!([{"id":"old-action","text":"Old task","done":true}]);
    old["recordings"] = json!([{"id":"retained","path":"/synthetic/original.wav"}]);
    let old = save(&lib, old);
    let prefs = json!({"summaryTemplate":"custom","templateInstructions":{"custom":"x".repeat(5000)},"model":"fixture"});
    lib.db
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO preferences(id,data) VALUES(1,?)",
            [prefs.to_string()],
        )
        .unwrap();
    let job = prepare(
        &lib,
        old["id"].as_str().unwrap(),
        "summary",
        Some("interview"),
    )
    .unwrap();
    assert_eq!(job.meeting["summaryTemplate"], old["summaryTemplate"]);
    let mut result = crate::summary_format::parse(
        r#"{"summary":[{"topic":"Review","text":"The review remains open."}],"actions":[]}"#,
    )
    .unwrap();
    result["summarySource"] = crate::summary_format::canonical().source();
    let saved = finish(&lib, &job, result, None).unwrap();
    assert_eq!(saved["summary"], "### Review\nThe review remains open.");
    assert_eq!(saved["summarySource"]["summaryFormatVersion"], 1);
    for field in [
        "summaryTemplate",
        "summaryInstructions",
        "notes",
        "transcript",
        "recordings",
    ] {
        assert_eq!(saved[field], old[field], "Lost legacy {field}");
    }
    let db = lib.db.lock().unwrap();
    assert_eq!(store::preferences(&db).unwrap(), prefs);
    let historic: String = db
        .query_row(
            "SELECT data FROM versions WHERE id=? AND revision=?",
            rusqlite::params![
                old["id"].as_str().unwrap(),
                old["revision"].as_i64().unwrap()
            ],
            |row| row.get(0),
        )
        .unwrap();
    let retained: Value = serde_json::from_str(&historic).unwrap();
    for field in ["summary", "summarySource", "actions", "recordings"] {
        assert_eq!(retained[field], old[field], "Changed historic {field}");
    }
    let export: Value = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    assert_eq!(export["summary"], saved["summary"]);
    assert_eq!(export["actions"], json!([]));
    assert_eq!(
        db.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
