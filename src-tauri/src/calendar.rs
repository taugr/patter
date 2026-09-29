//! Keep EventKit in the app process so macOS attributes consent to Patter.
use crate::store::Result;
use block2::RcBlock;
use chrono::{DateTime, SecondsFormat, Utc};
use objc2::{rc::Retained, runtime::Bool};
use objc2_event_kit::{
    EKAuthorizationStatus, EKEntityType, EKEventStatus, EKEventStore, EKParticipantStatus,
};
use objc2_foundation::{NSBundle, NSDate, NSError, NSString};
use serde_json::{json, Value};
use std::cell::RefCell;

thread_local! {
    // Accessed only on the main thread; retain the store throughout the consent sheet.
    static CONSENT_STORE: RefCell<Option<Retained<EKEventStore>>> = const { RefCell::new(None) };
}
fn status_name(status: EKAuthorizationStatus) -> &'static str {
    match status {
        EKAuthorizationStatus::NotDetermined => "not_requested",
        EKAuthorizationStatus::FullAccess => "allowed",
        EKAuthorizationStatus::Denied => "denied",
        EKAuthorizationStatus::Restricted => "restricted",
        EKAuthorizationStatus::WriteOnly => "write_only",
        _ => "unknown",
    }
}
fn packaged() -> bool {
    let bundle = NSBundle::mainBundle();
    bundle.bundlePath().to_string().ends_with(".app")
        && bundle
            .objectForInfoDictionaryKey(&NSString::from_str(
                "NSCalendarsFullAccessUsageDescription",
            ))
            .is_some()
}
#[tauri::command]
pub fn calendar_permission() -> &'static str {
    if !packaged() {
        return "unavailable";
    }
    // Querying status does not prompt or read calendar contents.
    status_name(unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) })
}
fn access_error(status: &str) -> String {
    match status {
        "not_requested" => "Click Connect calendar to let Patter request access. It appears in macOS Calendar permissions after requesting access.",
        "restricted" => "Calendar access is restricted by macOS or your administrator.",
        "unavailable" => "Calendar access needs the packaged Patter app. Open Patter from Applications, then connect again.",
        _ => "Allow full Calendar access for Patter in System Settings → Privacy & Security → Calendars, then click Connect calendar again.",
    }.into()
}
pub async fn request(app: &tauri::AppHandle) -> Result<()> {
    let status = calendar_permission();
    if status == "allowed" {
        return Ok(());
    }
    if !["not_requested", "write_only"].contains(&status) {
        return Err(access_error(status));
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        CONSENT_STORE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let store = slot.get_or_insert_with(|| unsafe { EKEventStore::new() });
            let tx = std::sync::Mutex::new(Some(tx));
            let completion = RcBlock::new(move |granted: Bool, error: *mut NSError| {
                let result = if !error.is_null() {
                    Err(unsafe { &*error }.localizedDescription().to_string())
                } else if granted.as_bool() { Ok(()) }
                else { Err(access_error("denied")) };
                if let Some(tx) = tx.lock().unwrap().take() { let _ = tx.send(result); }
            });
            // EventKit copies the completion block; the store lives until app exit.
            unsafe { store.requestFullAccessToEventsWithCompletion(RcBlock::as_ptr(&completion) as *mut _); }
        });
    }).map_err(|e| e.to_string())?;
    rx.await.map_err(|e| e.to_string())?
}
fn iso(timestamp: f64) -> Result<String> {
    DateTime::<Utc>::from_timestamp(timestamp as i64, 0)
        .map(|date| date.to_rfc3339_opts(SecondsFormat::Secs, true))
        .ok_or_else(|| "Calendar returned an invalid date.".into())
}
pub fn read() -> Result<Value> {
    let status = calendar_permission();
    if status != "allowed" {
        return Err(access_error(status));
    }
    // All retained EventKit objects are confined to this worker thread.
    unsafe {
        let store = EKEventStore::new();
        let now = Utc::now().timestamp() as f64;
        let from = NSDate::dateWithTimeIntervalSince1970(now);
        let until = NSDate::dateWithTimeIntervalSince1970(now + 14.0 * 86400.0);
        let predicate =
            store.predicateForEventsWithStartDate_endDate_calendars(&from, &until, None);
        let mut rows = Vec::new();
        for event in store.eventsMatchingPredicate(&predicate) {
            if event.isAllDay()
                || event.status() == EKEventStatus::Canceled
                || event.attendees().is_some_and(|people| {
                    people.iter().any(|person| {
                        person.isCurrentUser()
                            && person.participantStatus() == EKParticipantStatus::Declined
                    })
                })
            {
                continue;
            }
            let Some(calendar) = event.calendar() else {
                continue;
            };
            let start = iso(event.startDate().timeIntervalSince1970())?;
            let calendar_id = calendar.calendarIdentifier().to_string();
            let item = event
                .calendarItemExternalIdentifier()
                .unwrap_or_else(|| event.calendarItemIdentifier());
            let url = event
                .URL()
                .and_then(|url| url.absoluteString())
                .map(|url| url.to_string());
            let join_url = crate::meeting_links::find(&[
                url.clone(),
                event.location().map(|text| text.to_string()),
                event.notes().map(|text| text.to_string()),
            ]);
            rows.push(json!({
                "id": format!("{calendar_id}:{item}:{start}"),
                "title": event.title().to_string(),
                "start": start,
                "end": iso(event.endDate().timeIntervalSince1970())?,
                "calendar": calendar.title().to_string(),
                "calendarId": calendar_id,
                "url": url,
                "joinUrl": join_url,
            }));
        }
        rows.sort_by(|a, b| a["start"].as_str().cmp(&b["start"].as_str()));
        Ok(Value::Array(rows))
    }
}
#[tauri::command]
pub fn open_permission_settings(kind: String) -> Result<()> {
    let url = settings_url(&kind)?;
    std::process::Command::new("/usr/bin/open")
        .arg(url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn settings_url(kind: &str) -> Result<&'static str> {
    match kind {
        "calendar" => {
            Ok("x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars")
        }
        "notifications" => {
            Ok("x-apple.systempreferences:com.apple.Notifications-Settings.extension")
        }
        "accounts" => {
            Ok("x-apple.systempreferences:com.apple.Internet-Accounts-Settings.extension")
        }
        "microphone" => {
            Ok("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        }
        "screen" => {
            Ok("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        }
        _ => Err("Unknown permission setting.".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permission_states_have_distinct_recovery_paths() {
        assert_eq!(
            status_name(EKAuthorizationStatus::NotDetermined),
            "not_requested"
        );
        assert_eq!(status_name(EKAuthorizationStatus::Denied), "denied");
        assert_eq!(status_name(EKAuthorizationStatus::FullAccess), "allowed");
        assert_eq!(status_name(EKAuthorizationStatus::Restricted), "restricted");
        assert_eq!(status_name(EKAuthorizationStatus::WriteOnly), "write_only");
        assert!(access_error("not_requested").contains("Connect calendar"));
        assert!(access_error("denied").contains("System Settings"));
        assert!(access_error("unavailable").contains("packaged Patter"));
    }
    #[test]
    fn occurrence_dates_preserve_existing_swift_ids() {
        assert_eq!(iso(1790596800.0).unwrap(), "2026-09-28T12:00:00Z");
    }
    #[test]
    fn settings_links_cannot_open_arbitrary_urls_or_commands() {
        for kind in [
            "calendar",
            "notifications",
            "accounts",
            "microphone",
            "screen",
        ] {
            assert!(settings_url(kind)
                .unwrap()
                .starts_with("x-apple.systempreferences:"));
        }
        assert!(settings_url("https://example.com").is_err());
        assert!(settings_url("--args").is_err());
    }
}
