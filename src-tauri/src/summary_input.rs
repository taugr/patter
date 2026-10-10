//! Versioned text projection consumed by the summarizer. Derived freshness is never persisted.
use rusqlite::{params, Connection};
use serde_json::{json, Value};

pub const VERSION: u64 = 1;

pub fn project(meeting: &Value) -> String {
    let transcript = meeting["transcript"]
        .as_array()
        .map(|segments| {
            segments
                .iter()
                .filter_map(|segment| segment["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    format!(
        "Meeting: {}\nNotes:\n{}\nTranscript:\n{}",
        meeting["title"].as_str().unwrap_or(""),
        meeting["notes"].as_str().unwrap_or(""),
        transcript
    )
}

pub fn clear_derived(meeting: &mut Value) {
    if let Some(object) = meeting.as_object_mut() {
        object.remove("summaryInput");
        object.remove("summaryFreshness");
    }
}

/// Resolve the original conversation, including inherited provenance in draft copies.
/// Legacy, missing, unreadable or unsupported source versions remain unknown.
pub fn annotate(db: &Connection, mut meeting: Value) -> Value {
    clear_derived(&mut meeting);
    if meeting["summary"].as_str().is_none_or(str::is_empty) {
        return meeting;
    }
    let source = &meeting["summarySource"];
    let original = (|| {
        if source["inputProjectionVersion"].as_u64()? != VERSION {
            return None;
        }
        let id = source["sourceConversationId"].as_str()?;
        crate::store::valid_id(id).ok()?;
        let revision = source["sourceRevision"].as_i64().filter(|r| *r > 0)?;
        let raw: String = db
            .query_row(
                "SELECT data FROM versions WHERE id=? AND revision=?",
                params![id, revision],
                |row| row.get(0),
            )
            .ok()?;
        let retained: Value = serde_json::from_str(&raw).ok()?;
        if retained["id"] != id || retained["revision"] != revision {
            return None;
        }
        Some(project(&retained))
    })();
    meeting["summaryFreshness"] = json!(match original.as_ref() {
        Some(input) if *input == project(&meeting) => "current",
        Some(_) => "changed",
        None => "unknown",
    });
    if let Some(text) = original {
        meeting["summaryInput"] = json!({"projectionVersion":VERSION,"text":text});
    }
    meeting
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_matches_consumed_text_and_ignores_non_input_fields() {
        let meeting = json!({"title":"Review", "notes":"Friday", "transcript":[
            {"start":0,"speaker":"A","text":"First"},
            {"start":5,"text":"Second"}, {"start":8}
        ]});
        assert_eq!(
            project(&meeting),
            "Meeting: Review\nNotes:\nFriday\nTranscript:\nFirst\nSecond"
        );
        let mut changed = meeting.clone();
        changed["transcript"][0]["start"] = json!(25);
        changed["transcript"][0]["speaker"] = json!("B");
        changed["archived"] = json!(true);
        changed["actions"] = json!([{"done":true}]);
        assert_eq!(project(&meeting), project(&changed));
        changed["title"] = json!("Updated review");
        assert_ne!(project(&meeting), project(&changed));
    }
}
