//! Recognize conference links without exposing entire invitation notes to the UI.
use crate::store::Result;
use reqwest::Url;

pub fn normalize(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() > 8192 || value.chars().any(|c| c.is_control() || c == '\\') {
        return None;
    }
    let url = Url::parse(value).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return None;
    }
    let host = url.host_str()?;
    let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
    let recognized = if host == "meet.google.com" {
        (parts.len() == 1 && {
            let groups: Vec<_> = parts[0].split('-').collect();
            groups.len() == 3
                && groups.iter().zip([3, 4, 3]).all(|(group, len)| {
                    group.len() == len && group.bytes().all(|c| c.is_ascii_lowercase())
                })
        }) || (parts.len() == 2 && parts[0] == "lookup" && !parts[1].is_empty())
    } else if ["zoom.us", "zoom.com"]
        .iter()
        .any(|base| host == *base || host.ends_with(&format!(".{base}")))
    {
        (parts.len() == 2 && matches!(parts[0], "j" | "w") && numeric_id(parts[1]))
            || (parts.len() == 2 && parts[0] == "my" && !parts[1].is_empty())
            || (parts.len() == 3 && parts[0] == "wc" && numeric_id(parts[1]) && parts[2] == "join")
    } else {
        false
    };
    recognized.then(|| url.to_string())
}
fn numeric_id(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|c| c.is_ascii_digit())
}

pub fn find(fields: &[Option<String>]) -> Option<String> {
    for text in fields.iter().flatten() {
        // Invitations may contain plain text, HTML links, or Markdown links.
        let text = text
            .replace("&amp;", "&")
            .replace("&#38;", "&")
            .replace("&#x26;", "&");
        if let Some(url) = normalize(&text) {
            return Some(url);
        }
        for (start, _) in text.match_indices("https://") {
            let candidate = text[start..]
                .split(|c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\''))
                .next()
                .unwrap_or("")
                .trim_end_matches([')', ']', '}', '.', ',', ';', ':']);
            if let Some(url) = normalize(candidate) {
                return Some(url);
            }
        }
    }
    None
}

#[tauri::command]
pub async fn open_meeting_link(url: String) -> Result<()> {
    let url = normalize(&url).ok_or("This is not a supported Google Meet or Zoom meeting link.")?;
    tauri::async_runtime::spawn_blocking(move || {
        let status = std::process::Command::new("/usr/bin/open")
            .arg(url)
            .status()
            .map_err(|e| format!("Could not open the meeting: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("Could not open the meeting. Check your default browser and try again.".into())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_shared_conference_link_cases() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../../test-fixtures/meeting-links.json")).unwrap();
        for case in cases.as_array().unwrap() {
            let result = normalize(case["url"].as_str().unwrap());
            assert_eq!(
                result.is_some(),
                case["supported"].as_bool().unwrap(),
                "{}",
                case["url"]
            );
            if let Some(result) = result {
                assert_eq!(normalize(&result), Some(result));
            }
        }
    }
    #[test]
    fn extracts_links_in_field_order_and_preserves_passcodes() {
        let meet = "https://meet.google.com/abc-defg-hij";
        let zoom = "https://us02web.zoom.us/j/123456789?pwd=Encoded%2BPass&from=calendar";
        assert_eq!(
            find(&[Some(meet.into()), Some(zoom.into())]),
            Some(meet.into())
        );
        assert_eq!(
            find(&[
                Some("https://example.com/agenda".into()),
                Some(format!("Video call ({meet})."))
            ]),
            Some(meet.into())
        );
        assert_eq!(
            find(&[
                None,
                Some(format!(
                    "<a href=\"{}\">Join Zoom</a>",
                    zoom.replace('&', "&amp;")
                ))
            ]),
            Some(zoom.into())
        );
        assert_eq!(find(&[Some(format!("[Join]({meet})"))]), Some(meet.into()));
        assert_eq!(find(&[Some("Room 3, no video call".into()), None]), None);
    }
}
