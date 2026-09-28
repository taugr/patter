use super::{io, Config, Status};
use crate::store::Result;
use chrono::{DateTime, Duration, Local, NaiveTime};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
const LABEL: &str = "gr.tau.patter.backup";

pub fn parse_time(time: &str) -> Result<NaiveTime> {
    if time.len() != 5 || time.as_bytes()[2] != b':' {
        return Err("Choose a time in HH:MM format.".into());
    }
    NaiveTime::parse_from_str(time, "%H:%M").map_err(|_| "Choose a valid backup time.".into())
}
// Compare wall-clock dates/times so the first check after a skipped DST hour runs the backup.
// A repeated hour cannot run twice once that night's snapshot has completed.
pub fn due(c: &Config, s: &Status, now: DateTime<Local>) -> bool {
    if !c.enabled || !c.connected {
        return false;
    }
    let Ok(time) = parse_time(&c.time) else {
        return false;
    };
    if s.last_attempt
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .is_some_and(|last| now.signed_duration_since(last) < Duration::minutes(15))
    {
        return false;
    }
    if s.pending.is_some() {
        return true;
    }
    let day = if now.time() >= time {
        now.date_naive()
    } else {
        now.date_naive().pred_opt().unwrap()
    };
    // Completion can happen much later than capture; catch up after finishing an older pending run.
    !s.last_snapshot
        .as_ref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .is_some_and(|last| {
            let last = last.with_timezone(&Local).naive_local();
            last >= day.and_time(time)
        })
}
fn plist_path() -> Result<PathBuf> {
    Ok(
        PathBuf::from(std::env::var_os("HOME").ok_or("Home folder unavailable")?)
            .join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist")),
    )
}
fn domain() -> Result<String> {
    let out = Command::new("/usr/bin/id").arg("-u").output().map_err(io)?;
    let uid = String::from_utf8(out.stdout).map_err(io)?;
    if !out.status.success()
        || !uid.trim().bytes().all(|c| c.is_ascii_digit())
        || uid.trim().is_empty()
    {
        return Err("Could not identify the macOS login session.".into());
    }
    Ok(format!("gui/{}", uid.trim()))
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn plist(executable: &Path, time: &str) -> Result<String> {
    use chrono::Timelike;
    let time = parse_time(time)?;
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{LABEL}</string>
<key>ProgramArguments</key><array><string>{}</string><string>--backup-worker</string></array>
<key>StartCalendarInterval</key><dict><key>Hour</key><integer>{}</integer><key>Minute</key><integer>{}</integer></dict>
<key>StartInterval</key><integer>900</integer>
<key>RunAtLoad</key><true/>
<key>ProcessType</key><string>Background</string>
<key>LowPriorityIO</key><true/>
<key>Nice</key><integer>10</integer>
</dict></plist>
"#,
        escape(&executable.to_string_lossy()),
        time.hour(),
        time.minute()
    ))
}
pub fn status(_root: &Path) -> Value {
    let registered = domain().is_ok_and(|domain| {
        Command::new("/bin/launchctl")
            .args(["print", &format!("{domain}/{LABEL}")])
            .output()
            .is_ok_and(|o| o.status.success())
    });
    json!({"installed":plist_path().is_ok_and(|p|p.is_file()),"registered":registered})
}
pub fn install(_root: &Path, c: &Config) -> Result<()> {
    let path = plist_path()?;
    let domain = domain()?;
    let service = format!("{domain}/{LABEL}");
    let loaded = Command::new("/bin/launchctl")
        .args(["print", &service])
        .output()
        .map_err(io)?
        .status
        .success();
    if loaded {
        let result = Command::new("/bin/launchctl")
            .args(["bootout", &service])
            .output()
            .map_err(io)?;
        if !result.status.success() {
            return Err("Could not pause the existing backup schedule. Try again.".into());
        }
    }
    if !c.enabled {
        if path.exists() {
            fs::remove_file(&path).map_err(io)?;
        }
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(io)?;
    if !exe.to_string_lossy().contains(".app/Contents/MacOS/") {
        return Err("Enable nightly backups from the installed Patter app in Applications.".into());
    }
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    fs::create_dir_all(path.parent().unwrap()).map_err(io)?;
    let temporary = path.with_extension("plist.tmp");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(io)?;
    file.write_all(plist(&exe, &c.time)?.as_bytes())
        .map_err(io)?;
    file.sync_all().map_err(io)?;
    fs::rename(&temporary, &path).map_err(io)?;
    let result = Command::new("/bin/launchctl")
        .arg("bootstrap")
        .arg(&domain)
        .arg(&path)
        .output()
        .map_err(io)?;
    if !result.status.success() {
        return Err("macOS could not enable nightly backups. Check System Settings → General → Login Items and try again.".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    #[test]
    fn catches_up_and_throttles_retries() {
        let c = Config {
            enabled: true,
            connected: true,
            ..Config::default()
        };
        let now = Local.with_ymd_and_hms(2026, 9, 24, 8, 0, 0).unwrap();
        let mut s = Status::default();
        assert!(due(&c, &s, now));
        s.last_snapshot = Some(now.to_rfc3339());
        assert!(!due(&c, &s, now));
        s.pending = Some("pending".into());
        assert!(due(&c, &s, now));
        s.last_attempt = Some((now - Duration::minutes(5)).to_rfc3339());
        assert!(!due(&c, &s, now));
        s.last_attempt = None;
        s.pending = None;
        s.last_snapshot = Some((now - Duration::days(2)).to_rfc3339());
        s.last_success = Some(now.to_rfc3339());
        assert!(due(&c, &s, now));
    }
    #[test]
    fn emits_safe_calendar_and_catchup_schedule() {
        let p = plist(
            Path::new("/Applications/Tom & Patter.app/Contents/MacOS/patter"),
            "02:35",
        )
        .unwrap();
        assert!(p.contains("Tom &amp; Patter"));
        assert!(p.contains("<integer>35</integer>"));
        assert!(p.contains("--backup-worker"));
        assert!(p.contains("StartInterval"));
        assert!(parse_time("25:00").is_err());
        assert!(parse_time("2:00").is_err());
    }
}
