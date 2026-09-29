//! Use the capture helper's consent path without opening a stream or library item.
use crate::{activity::Activity, store::Result};
use objc2_foundation::NSBundle;
use std::{
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Manager, State};

static REQUEST: Mutex<()> = Mutex::new(());

fn packaged() -> bool {
    NSBundle::mainBundle()
        .bundlePath()
        .to_string()
        .ends_with(".app")
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum MicrophonePermission {
    Allowed,
    NotRequested,
    Denied,
    Restricted,
    Unknown,
}
#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RecordingPermissions {
    microphone: MicrophonePermission,
    screen_allowed: bool,
}

#[tauri::command]
pub async fn recording_permissions(
    app: tauri::AppHandle,
    activity: State<'_, Activity>,
) -> Result<Option<RecordingPermissions>> {
    if !packaged() {
        return Ok(None);
    }
    let output = run_helper(app, activity, "recording-permissions", 10).await?;
    serde_json::from_slice(&output)
        .map(Some)
        .map_err(|_| "Recording permission status could not be read.".into())
}

#[tauri::command]
pub async fn request_capture_permission(
    app: tauri::AppHandle,
    activity: State<'_, Activity>,
    kind: String,
) -> Result<()> {
    let mode = match kind.as_str() {
        "microphone" => "request-microphone-access",
        "screen" => "request-screen-access",
        _ => return Err("Unknown recording permission.".into()),
    };
    let output = run_helper(app, activity, mode, 120).await?;
    parse_response(true, &output)
}

#[tauri::command]
pub async fn request_recording_access(
    app: tauri::AppHandle,
    activity: State<'_, Activity>,
) -> Result<()> {
    let output = run_helper(app, activity, "request-recording-access", 120).await?;
    parse_response(true, &output)
}

async fn run_helper(
    app: tauri::AppHandle,
    activity: State<'_, Activity>,
    mode: &'static str,
    timeout: u64,
) -> Result<Vec<u8>> {
    if !packaged() {
        return Err("Open the packaged Patter app to enable recording access.".into());
    }
    let job = activity.job()?;
    let helper = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("resources/patter-native");
    tauri::async_runtime::spawn_blocking(move || {
        let _job = job;
        let _request = REQUEST.try_lock().map_err(|_| "A recording access request is already open.")?;
        let mut child = Command::new(helper)
            .arg(mode)
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())
            .spawn().map_err(|e| e.to_string())?;
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if started.elapsed() < Duration::from_secs(timeout) => std::thread::sleep(Duration::from_millis(100)),
                result => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(match result {
                        Err(e) => e.to_string(),
                        _ => "Recording access was not confirmed in time. Complete the macOS permission prompts, then try again.".into(),
                    });
                }
            }
        }
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() { parse_response(false, &output.stdout)?; }
        Ok(output.stdout)
    }).await.map_err(|e| e.to_string())?
}

fn parse_response(success: bool, output: &[u8]) -> Result<()> {
    let response: serde_json::Value = serde_json::from_slice(output).map_err(|_| {
        "Recording access could not be confirmed. Try again from the installed Patter app."
    })?;
    if let Some(error) = response["error"].as_str() {
        return Err(error.into());
    }
    if success && response["status"] == "access-granted" {
        Ok(())
    } else {
        Err(
            "Recording access could not be confirmed. Try again from the installed Patter app."
                .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_requires_explicit_known_values() {
        assert!(serde_json::from_str::<RecordingPermissions>(
            r#"{"microphone":"denied","screenAllowed":false}"#
        )
        .is_ok());
        for invalid in [
            r#"{}"#,
            r#"{"microphone":"allowed"}"#,
            r#"{"microphone":"bad","screenAllowed":true}"#,
        ] {
            assert!(serde_json::from_str::<RecordingPermissions>(invalid).is_err());
        }
    }
    #[test]
    fn only_successful_consent_is_reported_as_allowed() {
        assert!(parse_response(true, br#"{"status":"access-granted"}"#).is_ok());
        assert!(parse_response(false, br#"{"status":"access-granted"}"#).is_err());
        assert!(parse_response(true, br#"{"status":"recording"}"#).is_err());
        assert!(parse_response(true, b"").is_err());
        assert_eq!(
            parse_response(false, br#"{"error":"Allow Microphone access"}"#).unwrap_err(),
            "Allow Microphone access"
        );
    }
}
