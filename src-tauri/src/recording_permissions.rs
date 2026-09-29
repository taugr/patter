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

#[tauri::command]
pub async fn request_recording_access(
    app: tauri::AppHandle,
    activity: State<'_, Activity>,
) -> Result<()> {
    if !NSBundle::mainBundle()
        .bundlePath()
        .to_string()
        .ends_with(".app")
    {
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
            .arg("request-recording-access")
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())
            .spawn().map_err(|e| e.to_string())?;
        let started = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if started.elapsed() < Duration::from_secs(120) => std::thread::sleep(Duration::from_millis(100)),
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
        parse_response(output.status.success(), &output.stdout)
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
