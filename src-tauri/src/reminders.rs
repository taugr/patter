use crate::{
    notifications,
    store::{Library, Result},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, State};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub title: String,
    pub start: String,
    pub end: String,
    pub calendar: String,
    pub calendar_id: String,
    pub url: Option<String>,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    events: Vec<Event>,
    alerts: Vec<Event>,
    requested: Option<String>,
    error: Option<String>,
    notification_error: Option<String>,
    last_updated: Option<String>,
}
#[derive(Default)]
pub struct Reminders(Mutex<Snapshot>);
fn time(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value)
        .map(|d| d.timestamp())
        .unwrap_or(0)
}
fn minutes(p: &Value) -> i64 {
    p["reminderMinutes"]
        .as_i64()
        .filter(|v| [0, 1, 2, 5, 10, 15, 30].contains(v))
        .unwrap_or(5)
}
fn enabled(p: &Value) -> bool {
    p["calendarEnabled"] == true && p["reminderEnabled"] == true
}
fn calendar_selected(event: &Event, p: &Value) -> bool {
    p["reminderCalendars"].as_array().is_none_or(|ids| {
        ids.is_empty() || ids.iter().any(|id| id.as_str() == Some(&event.calendar_id))
    })
}
fn due(event: &Event, p: &Value, now: i64, seen: &HashMap<String, i64>) -> bool {
    let start = time(&event.start);
    enabled(p)
        && start > 0
        && now >= start - minutes(p) * 60
        && now < start + 60
        && now < time(&event.end)
        && !seen.contains_key(&event.id)
        && calendar_selected(event, p)
}
pub fn validate(p: &Value) -> Result<()> {
    if let Some(value) = p.get("reminderMinutes") {
        if value
            .as_i64()
            .is_none_or(|v| ![0, 1, 2, 5, 10, 15, 30].contains(&v))
        {
            return Err("Choose a reminder time from 0 to 30 minutes.".into());
        }
    }
    if let Some(ids) = p.get("reminderCalendars") {
        if ids.as_array().is_none_or(|ids| {
            ids.len() > 100
                || ids
                    .iter()
                    .any(|id| id.as_str().is_none_or(|id| id.len() > 1024))
        }) {
            return Err("Invalid reminder calendars.".into());
        }
    }
    Ok(())
}
pub fn activate(app: &tauri::AppHandle, id: &str, record: bool) {
    if let Some(state) = app.try_state::<Reminders>() {
        let mut state = state.0.lock().unwrap();
        if record
            && state
                .alerts
                .iter()
                .any(|e| e.id == id && time(&e.end) > Utc::now().timestamp())
        {
            state.requested = Some(id.into());
        }
    }
}
#[tauri::command]
pub fn reminder_status(state: State<Reminders>) -> Value {
    let mut state = state.0.lock().unwrap();
    let value = serde_json::to_value(&*state).unwrap_or(json!({}));
    state.requested = None;
    value
}
#[tauri::command]
pub fn dismiss_reminder(state: State<Reminders>, id: String) {
    state.0.lock().unwrap().alerts.retain(|e| e.id != id);
    notifications::remove(&id);
}
#[tauri::command]
pub async fn test_reminder(state: State<'_, Reminders>) -> Result<()> {
    if notifications::permission().await? != "allowed" {
        return Err("Allow Patter notifications in macOS System Settings → Notifications.".into());
    }
    let now = Utc::now();
    let event = Event {
        id: "patter-test".into(),
        title: "Example meeting".into(),
        start: (now + chrono::Duration::minutes(5)).to_rfc3339(),
        end: (now + chrono::Duration::minutes(10)).to_rfc3339(),
        calendar: "Test reminder".into(),
        calendar_id: "test".into(),
        url: None,
    };
    {
        let mut state = state.0.lock().unwrap();
        state.alerts.retain(|e| e.id != "patter-test");
        state.alerts.push(event);
    }
    notifications::show(
        "patter-test",
        "Patter reminder",
        "Meeting reminders are ready. This test will not record audio.",
        false,
    )
    .await
}
fn preferences(app: &tauri::AppHandle) -> Result<Value> {
    crate::store::preferences(&*crate::lock_db(&app.state::<Library>())?)
}
pub fn start(app: &tauri::AppHandle) {
    notifications::initialize(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let path = app.state::<Library>().root.join("reminder-deliveries.json");
        let mut seen: HashMap<String, i64> = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let mut last_refresh: Option<Instant> = None;
        let mut previous = Value::Null;
        loop {
            let p = preferences(&app).unwrap_or(Value::Null);
            if p["calendarEnabled"] == true {
                if previous != p
                    || last_refresh.is_none_or(|at| at.elapsed() >= Duration::from_secs(60))
                {
                    last_refresh = Some(Instant::now());
                    let fetched = crate::read_calendar(&app, false).await.and_then(|v| {
                        serde_json::from_value::<Vec<Event>>(v).map_err(|e| e.to_string())
                    });
                    let state = app.state::<Reminders>();
                    let mut state = state.0.lock().unwrap();
                    match fetched {
                        Ok(events) => {
                            state.events = events;
                            state.error = None;
                            state.last_updated = Some(Utc::now().to_rfc3339());
                        }
                        Err(e) => {
                            state.error = Some(e);
                            state.events.clear();
                        }
                    }
                    let _ = app.emit("patter-calendar-refreshed", &state.events);
                }
            } else {
                app.state::<Reminders>().0.lock().unwrap().events.clear();
                last_refresh = None;
            }
            previous = p.clone();
            let p = preferences(&app).unwrap_or(Value::Null);
            let now = Utc::now().timestamp();
            seen.retain(|_, end| *end > now - 86400);
            let due_events = {
                let state = app.state::<Reminders>();
                let mut state = state.0.lock().unwrap();
                let events = state.events.clone();
                state.alerts.retain(|alert| {
                    if alert.id == "patter-test" && time(&alert.end) > now {
                        return true;
                    }
                    let keep = enabled(&p)
                        && calendar_selected(alert, &p)
                        && time(&alert.end) > now
                        && events.iter().any(|event| event.id == alert.id);
                    if !keep {
                        notifications::remove(&alert.id);
                    }
                    keep
                });
                if !enabled(&p) {
                    if state.requested.as_deref() != Some("patter-test") {
                        state.requested = None;
                    }
                    state.notification_error = None;
                }
                events
                    .into_iter()
                    .filter(|e| due(e, &p, now, &seen))
                    .collect::<Vec<_>>()
            };
            for event in due_events {
                // Persist before dispatch so app restarts cannot repeat a delivered reminder.
                seen.insert(event.id.clone(), time(&event.end));
                let saved = serde_json::to_vec(&seen)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| crate::agent::transport::write_private(&path, &bytes));
                if let Err(e) = saved {
                    app.state::<Reminders>()
                        .0
                        .lock()
                        .unwrap()
                        .notification_error = Some(e);
                    continue;
                }
                app.state::<Reminders>()
                    .0
                    .lock()
                    .unwrap()
                    .alerts
                    .push(event.clone());
                let title = if p["reminderShowTitle"] == false {
                    "Meeting starting soon"
                } else {
                    &event.title
                };
                let remaining = (time(&event.start) - now + 59).max(0) / 60;
                let body = if remaining > 0 {
                    format!("Starts in {remaining} min. Open Patter to take notes or record.")
                } else {
                    "Starting now. Open Patter to take notes or record.".into()
                };
                let result = match notifications::permission().await {
                    Ok(permission) if permission == "allowed" && enabled(&preferences(&app).unwrap_or(Value::Null)) => notifications::show(&event.id, title, &body, p["reminderSound"] != false).await,
                    Ok(_) => Err("Allow Patter in System Settings → Notifications. In-app reminders remain available.".into()),
                    Err(e) => Err(e),
                };
                app.state::<Reminders>()
                    .0
                    .lock()
                    .unwrap()
                    .notification_error = result.err();
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    fn event() -> Event {
        serde_json::from_value(json!({"id":"a","title":"Meeting","start":"2026-09-28T12:00:00Z","end":"2026-09-28T13:00:00Z","calendar":"Work","calendarId":"work","url":null})).unwrap()
    }
    #[test]
    fn timing_filters_and_restart_deduplication() {
        let e = event();
        let p = json!({"calendarEnabled":true,"reminderEnabled":true,"reminderMinutes":5});
        let now = time(&e.start);
        assert!(!due(&e, &p, now - 301, &HashMap::new()));
        assert!(due(&e, &p, now - 300, &HashMap::new()));
        assert!(due(&e, &p, now + 30, &HashMap::new()));
        assert!(!due(&e, &p, now + 60, &HashMap::new()));
        let seen = HashMap::from([(e.id.clone(), time(&e.end))]);
        let restored = serde_json::from_str(&serde_json::to_string(&seen).unwrap()).unwrap();
        assert!(!due(&e, &p, now, &restored));
        assert!(!due(
            &e,
            &json!({"calendarEnabled":true,"reminderEnabled":false}),
            now,
            &HashMap::new()
        ));
        assert!(!due(
            &e,
            &json!({"calendarEnabled":true,"reminderEnabled":true,"reminderCalendars":["personal"]}),
            now,
            &HashMap::new()
        ));
    }
    #[test]
    fn changed_occurrence_and_invalid_preferences() {
        let mut e = event();
        let seen = HashMap::from([(e.id.clone(), time(&e.end))]);
        e.id = "rescheduled".into();
        assert!(due(
            &e,
            &json!({"calendarEnabled":true,"reminderEnabled":true}),
            time(&e.start),
            &seen
        ));
        assert!(validate(&json!({"reminderMinutes":-1})).is_err());
        assert!(validate(&json!({"reminderCalendars":"work"})).is_err());
    }
    #[test]
    fn calendar_selection_applies_to_new_and_visible_reminders() {
        let e = event();
        assert!(calendar_selected(&e, &json!({})));
        assert!(calendar_selected(&e, &json!({"reminderCalendars":[]})));
        assert!(calendar_selected(
            &e,
            &json!({"reminderCalendars":["work"]})
        ));
        assert!(!calendar_selected(
            &e,
            &json!({"reminderCalendars":["personal"]})
        ));
    }
}
