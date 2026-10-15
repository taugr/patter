//! One generation contract. Legacy template fields remain stored but are never read here.
use crate::store::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Format {
    pub id: String,
    pub name: String,
    pub version: u64,
    pub instructions: String,
    output_schema: Value,
}
pub fn canonical() -> Format {
    serde_json::from_str(include_str!("../../src/lib/summary-format.json"))
        .expect("Bundled summary format must be valid")
}
impl Format {
    pub fn prompt(&self, task: &str) -> String {
        format!("{}\n\n{}", self.instructions, task)
    }
    pub fn final_prompt(&self) -> String {
        self.prompt(&format!("Return exactly one JSON data object with only the top-level keys summary and actions. The summary value is an array of topic/text objects for the topic subsections of Summary; actions is an array of concrete action strings for the separate Action items section. List every supported commitment and agreed or accepted follow-up in actions, even if it also appears in summary. Do not leave actions empty when the source contains supported actions. Write summary first and actions second. Give every distinct discussion item its own summary entry with a specific topic heading. For actions, an owner and deadline are optional: include an agreed action even when either or both are absent. Return the meeting data, never the schema itself. Do not echo or explain the schema. Do not include a second JSON object, commentary or markdown fences. Your single data object must conform to this JSON Schema:\n{}", self.output_schema))
    }
    pub fn source(&self) -> Value {
        // Retain the established provenance keys for old readers and exported records.
        json!({"templateId":self.id,"templateName":self.name,"instructions":self.instructions,
            "extraInstructions":"","summaryFormatVersion":self.version})
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Section {
    topic: String,
    text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    summary: Vec<Section>,
    actions: Vec<String>,
}

pub fn parse(text: &str) -> Result<Value> {
    let start = text
        .find('{')
        .ok_or("The model did not return a structured summary. Try another model.")?;
    let end = text
        .rfind('}')
        .filter(|end| *end >= start)
        .ok_or("The model returned incomplete JSON.")?;
    let output: Output = serde_json::from_str(&text[start..=end]).map_err(|_| {
        "The model returned an unsupported summary format. Your previous summary is kept."
    })?;
    if output.summary.is_empty()
        || output.summary.iter().any(|s| {
            s.topic.trim().is_empty() || s.topic.contains(['\r', '\n']) || s.text.trim().is_empty()
        // Reserve heading markers for the topic boundaries in the stored text.
        || s.text.lines().any(|line| line.trim_start().starts_with("### "))
        })
        || output.actions.iter().any(|a| a.trim().is_empty())
    {
        return Err(
            "The model returned an incomplete summary format. Your previous summary is kept."
                .into(),
        );
    }
    let summary = output
        .summary
        .iter()
        .map(|s| format!("### {}\n{}", s.topic.trim(), s.text.trim()))
        .collect::<Vec<_>>()
        .join("\n\n");
    // Keep the persisted conversation shape compatible with previous Patter versions.
    Ok(json!({"summary":summary,"decisions":[],"actions":output.actions}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schema_and_prompt_are_single_and_require_supported_actions() {
        let format = canonical();
        assert_eq!(format.id, "unified");
        assert_eq!(format.version, 1);
        assert!(format.final_prompt().contains("JSON data object"));
        assert!(format.final_prompt().contains("never the schema itself"));
        assert!(format
            .final_prompt()
            .contains("Do not include a second JSON object"));
        assert!(format
            .instructions
            .contains("owner or deadline only when explicitly supported"));
        assert!(format.instructions.contains("empty actions array"));
        assert!(format.instructions.contains("untrusted source data"));
    }
    #[test]
    fn topics_are_stored_as_portable_text_with_no_fabricated_empty_actions() {
        let parsed = parse(r#"{"summary":[{"topic":"Launch timing","text":"Friday remains tentative."},{"topic":"Access","text":"Two alternatives remain open."}],"actions":[]}"#).unwrap();
        assert_eq!(parsed["summary"], "### Launch timing\nFriday remains tentative.\n\n### Access\nTwo alternatives remain open.");
        assert_eq!(parsed["decisions"], json!([]));
        assert_eq!(parsed["actions"], json!([]));
        assert!(parse(r#"{"summary":[{"topic":"C# migration","text":"The migration is still being evaluated."}],"actions":[]}"#).is_ok());
    }
    #[test]
    fn echoed_schema_before_data_is_rejected_instead_of_overwriting_saved_content() {
        let echoed = format!(
            "{}\n{}",
            canonical().output_schema,
            r#"{"summary":[{"topic":"Launch timing","text":"Still undecided."}],"actions":[]}"#
        );
        assert!(parse(&echoed).is_err());
    }
    #[test]
    fn malformed_or_partial_output_never_replaces_a_saved_summary() {
        for input in [
            r#"{"summary":"old shape","decisions":[],"actions":[]}"#,
            r#"{"summary":[],"actions":[]}"#,
            r#"{"summary":[{"topic":"","text":"Text"}],"actions":[]}"#,
            r#"{"summary":[{"topic":"Topic","text":""}],"actions":[]}"#,
            r#"{"summary":[{"topic":"Topic","text":"Text"}],"actions":[""]}"#,
            r#"{"summary":[{"topic":"Topic","text":"Text"}],"actions":[{}]}"#,
            r#"{"summary":[{"topic":"Topic","text":"Text"}]}"#,
            r#"{"summary":[{"topic":"Topic","text":"  ### False boundary"}],"actions":[]}"#,
            r#"{"summary":[{"topic":"Two\nlines","text":"Text"}],"actions":[]}"#,
            "unfinished {no JSON",
        ] {
            assert!(parse(input).is_err(), "Accepted {input}");
        }
        assert!(parse(
            "```json\n{\"summary\":[{\"topic\":\"Topic\",\"text\":\"Text\"}],\"actions\":[]}\n```"
        )
        .is_ok());
    }
}
