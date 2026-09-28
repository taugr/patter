use super::{atomic_json, hash_file, io, read_json, safe_relative, Config};
use crate::store::Result;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    net::TcpListener,
    path::Path,
    time::{Duration, Instant},
};
const SERVICE: &str = "gr.tau.patter.google-drive";
const API: &str = "https://www.googleapis.com/drive/v3";

#[derive(Serialize, Deserialize)]
struct Credentials {
    client_id: String,
    client_secret: String,
    refresh_token: String,
}
fn client() -> Result<Client> {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(io)
}
fn send(r: reqwest::blocking::RequestBuilder) -> Result<Response> {
    r.send().map_err(|_| {
        "Could not reach Google Drive. Check your connection; the backup can resume.".into()
    })
}
fn checked(r: Response) -> Result<Response> {
    if r.status().is_success() {
        Ok(r)
    } else {
        Err(match r.status().as_u16() {
            401 => "Google sign-in expired. Reconnect Google Drive.".into(),
            403 => "Google refused access. Check permissions and Drive storage quota.".into(),
            404 => "A backup file or folder is missing from Google Drive.".into(),
            429 => "Google Drive is busy. The backup will retry later.".into(),
            n => format!("Google returned HTTP {n}. The backup has not been marked complete."),
        })
    }
}
fn value(r: Response) -> Result<Value> {
    checked(r)?
        .json()
        .map_err(|_| "Google returned an unreadable response.".into())
}
// Keychain interaction policy is process-wide. Serialize every credential operation
// so one guard cannot re-enable prompts while another operation is still running.
static KEYCHAIN_ACCESS: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn without_keychain_prompts<T>(work: impl FnOnce() -> Result<T>) -> Result<T> {
    let _access = KEYCHAIN_ACCESS
        .lock()
        .map_err(|_| "Keychain access is busy.")?;
    let _no_prompts =
        security_framework::os::macos::keychain::SecKeychain::disable_user_interaction()
            .map_err(io)?;
    work()
}
fn credential_account(c: &Config) -> &str {
    if c.credential_account.is_empty() {
        &c.library_id
    } else {
        &c.credential_account
    }
}
fn credentials(c: &Config) -> Result<Credentials> {
    without_keychain_prompts(|| {
        let raw = security_framework::passwords::get_generic_password(SERVICE, credential_account(c))
            .map_err(|_| "Google credentials are unavailable. Unlock your login Keychain, or choose Reconnect Google Drive and select your Google setup JSON again.")?;
        serde_json::from_slice(&raw)
            .map_err(|_| "Reconnect Google Drive to renew credentials.".into())
    })
}
pub fn credentials_available(c: &Config) -> bool {
    credentials(c).is_ok()
}
fn rotate_credential_account(c: &mut Config, save: impl FnOnce(&str) -> Result<()>) -> Result<()> {
    // An updated ad-hoc binary cannot overwrite the previous binary's protected
    // item. Store newly authorized credentials under a fresh account instead.
    // Do not widen the old ACL, remove the old item, or change the library identity.
    let account = uuid::Uuid::new_v4().to_string();
    save(&account)?;
    c.credential_account = account;
    Ok(())
}
fn save_credentials(c: &mut Config, secret: &Credentials) -> Result<()> {
    rotate_credential_account(c, |account| {
        without_keychain_prompts(|| {
            security_framework::passwords::set_generic_password(
                SERVICE,
                account,
                &serde_json::to_vec(secret).map_err(io)?,
            )
            .map_err(|_| {
                "Could not save Google credentials. Unlock your login Keychain and try again."
                    .into()
            })
        })
    })
}
pub fn disconnect(c: &Config) -> Result<()> {
    without_keychain_prompts(|| {
        match security_framework::passwords::delete_generic_password(SERVICE, credential_account(c)) {
        Ok(()) => Ok(()),
        Err(e) if e.code() == -25300 => Ok(()),
        Err(_) => Err("Could not remove Google credentials from Keychain. Unlock your login Keychain and try again.".into()),
    }
    })
}
pub fn valid_id(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 256
        || !s
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        Err("Invalid Google Drive file identifier.".into())
    } else {
        Ok(())
    }
}
fn random() -> String {
    format!(
        "{}{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
fn callback(request: &str, state: &str) -> Option<Result<String>> {
    let mut words = request.split_whitespace();
    if words.next() != Some("GET") {
        return None;
    }
    let target = words.next()?;
    if !target.starts_with("/callback?") {
        return None;
    }
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    let pairs = url
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    if pairs.get("state").map(|s| s.as_ref()) != Some(state) {
        return None;
    }
    if pairs.contains_key("error") {
        return Some(Err("Google connection was cancelled or denied.".into()));
    }
    pairs.get("code").map(|s| Ok(s.to_string()))
}
fn read_callback_line(stream: &mut std::net::TcpStream) -> Result<String> {
    // Accepted sockets can inherit the listener's nonblocking mode on macOS.
    // Read the complete request line even when the browser sends it in multiple packets.
    stream.set_nonblocking(false).map_err(io)?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(io)?;
    let mut line = String::new();
    BufReader::new(stream)
        .take(8192)
        .read_line(&mut line)
        .map_err(|_| "Could not read the Google sign-in callback")?;
    if !line.ends_with('\n') {
        return Err("Incomplete Google sign-in callback".into());
    }
    Ok(line)
}
pub fn connect(c: &mut Config, path: Option<&Path>) -> Result<()> {
    let mut secret = if let Some(path) = path {
        if fs::metadata(path).map_err(io)?.len() > 65536 {
            return Err("Choose a Google Desktop OAuth credentials JSON file.".into());
        }
        let data: Value = serde_json::from_slice(&fs::read(path).map_err(io)?)
            .map_err(|_| "Invalid OAuth credentials JSON")?;
        let desktop = &data["installed"];
        let id = desktop["client_id"]
            .as_str()
            .filter(|s| s.ends_with(".apps.googleusercontent.com"))
            .ok_or("Choose credentials for a Desktop app, not a web app or service account.")?;
        Credentials {
            client_id: id.into(),
            client_secret: desktop["client_secret"]
                .as_str()
                .ok_or("Missing desktop client secret")?
                .into(),
            refresh_token: String::new(),
        }
    } else {
        credentials(c)?
    };
    let listener = TcpListener::bind("127.0.0.1:0").map_err(io)?;
    listener.set_nonblocking(true).map_err(io)?;
    let redirect = format!(
        "http://127.0.0.1:{}/callback",
        listener.local_addr().map_err(io)?.port()
    );
    let state = random();
    let verifier = random();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let mut url = reqwest::Url::parse("https://accounts.google.com/o/oauth2/v2/auth").unwrap();
    url.query_pairs_mut().extend_pairs([
        ("client_id", secret.client_id.as_str()),
        ("redirect_uri", &redirect),
        ("response_type", "code"),
        ("scope", "https://www.googleapis.com/auth/drive.file"),
        ("access_type", "offline"),
        ("prompt", "consent select_account"),
        ("state", &state),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
    ]);
    let opened = std::process::Command::new("/usr/bin/open")
        .arg(url.as_str())
        .status()
        .map_err(io)?;
    if !opened.success() {
        return Err("Could not open Google sign-in in your browser.".into());
    }
    let deadline = Instant::now() + Duration::from_secs(180);
    let code = loop {
        if Instant::now() > deadline {
            return Err("Google sign-in timed out. Choose Connect to try again.".into());
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let result = read_callback_line(&mut stream)
                    .ok()
                    .and_then(|line| callback(&line, &state));
                let body = if result.is_some() {
                    "Return to Patter to finish connecting Google Drive."
                } else {
                    "This sign-in response was not accepted."
                };
                let status = if result.is_some() {
                    "200 OK"
                } else {
                    "400 Bad Request"
                };
                let _=write!(stream,"HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                if let Some(result) = result {
                    break result?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(e) => return Err(io(e)),
        }
    };
    let http = client()?;
    let response = value(send(
        http.post("https://oauth2.googleapis.com/token").form(&[
            ("client_id", secret.client_id.as_str()),
            ("client_secret", secret.client_secret.as_str()),
            ("code", &code),
            ("redirect_uri", &redirect),
            ("grant_type", "authorization_code"),
            ("code_verifier", &verifier),
        ]),
    )?)?;
    secret.refresh_token = response["refresh_token"]
        .as_str()
        .ok_or("Google did not grant offline access. Reconnect and allow Drive access.")?
        .into();
    let access = response["access_token"]
        .as_str()
        .ok_or("Google did not return an access token")?
        .to_owned();
    let mut drive = Drive {
        base: "https://www.googleapis.com".into(),
        http,
        secret,
        access,
        expires: Instant::now() + Duration::from_secs(3000),
    };
    let token = drive.token()?;
    let about = value(send(
        drive
            .http
            .get(format!("{API}/about"))
            .bearer_auth(token)
            .query(&[("fields", "user(emailAddress)")]),
    )?)?;
    let email = about["user"]["emailAddress"]
        .as_str()
        .ok_or("Could not identify the connected Google account")?
        .to_owned();
    if c.connected && (c.client_id != drive.secret.client_id || c.email != email) {
        return Err("Reconnect using the same Google account and setup file so your existing backup can resume.".into());
    }
    // Reuse a folder created by this OAuth app, including on another Mac. Never inspect unrelated files.
    let roots=drive.list("trashed=false and mimeType='application/vnd.google-apps.folder' and appProperties has { key='patterRoot' and value='1' }")?;
    let folder = if let Some(id) = roots.first().and_then(|v| v["id"].as_str()) {
        id.to_owned()
    } else {
        let token = drive.token()?;
        let v=value(send(drive.http.post(format!("{API}/files")).bearer_auth(token).query(&[("fields","id")]).json(&json!({"name":"Patter Backups","mimeType":"application/vnd.google-apps.folder","appProperties":{"patterRoot":"1"}})))?)?;
        v["id"]
            .as_str()
            .ok_or("Google did not create the backup folder")?
            .into()
    };
    save_credentials(c, &drive.secret)?;
    c.client_id = drive.secret.client_id.clone();
    c.email = email;
    c.folder_id = folder;
    c.connected = true;
    Ok(())
}

pub struct Drive {
    base: String,
    http: Client,
    secret: Credentials,
    access: String,
    expires: Instant,
}
impl Drive {
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }
    fn validate_upload_url(&self, url: &str) -> Result<()> {
        #[cfg(test)]
        if self.base.starts_with("http://127.0.0.1:")
            && url.starts_with(&format!("{}/upload/drive/v3/files?", self.base))
        {
            return Ok(());
        }
        valid_upload_url(url)
    }

    pub fn new(c: &Config) -> Result<Self> {
        if !c.connected {
            return Err("Connect Google Drive first.".into());
        }
        Ok(Self {
            base: "https://www.googleapis.com".into(),
            http: client()?,
            secret: credentials(c)?,
            access: String::new(),
            expires: Instant::now(),
        })
    }
    fn token(&mut self) -> Result<String> {
        if self.access.is_empty() || Instant::now() >= self.expires {
            let response = send(
                self.http
                    .post("https://oauth2.googleapis.com/token")
                    .form(&[
                        ("client_id", self.secret.client_id.as_str()),
                        ("client_secret", self.secret.client_secret.as_str()),
                        ("refresh_token", self.secret.refresh_token.as_str()),
                        ("grant_type", "refresh_token"),
                    ]),
            )?;
            if !response.status().is_success() {
                return Err(
                    "Google authorization needs attention. Reconnect Google Drive in Settings."
                        .into(),
                );
            }
            let v: Value = response
                .json()
                .map_err(|_| "Invalid Google token response")?;
            self.access = v["access_token"]
                .as_str()
                .ok_or("Google returned no access token")?
                .into();
            self.expires = Instant::now()
                + Duration::from_secs(v["expires_in"].as_u64().unwrap_or(3600).saturating_sub(60));
        }
        Ok(self.access.clone())
    }
    fn list(&mut self, q: &str) -> Result<Vec<Value>> {
        let mut out = vec![];
        let mut page = String::new();
        loop {
            let token = self.token()?;
            let v=value(send(self.http.get(self.url("/drive/v3/files")).bearer_auth(token).query(&[("q",q),("spaces","drive"),("fields","nextPageToken,files(id,name,size,sha256Checksum,appProperties,createdTime)"),("pageSize","1000"),("pageToken",page.as_str())]))?)?;
            out.extend(
                v["files"]
                    .as_array()
                    .ok_or("Invalid Drive file list")?
                    .iter()
                    .cloned(),
            );
            if let Some(next) = v["nextPageToken"].as_str() {
                page = next.into();
            } else {
                break;
            }
        }
        Ok(out)
    }
    pub fn folder(&mut self, parent: &str, name: &str, key: &str) -> Result<String> {
        valid_id(parent)?;
        let key = format!("{:x}", Sha256::digest(key.as_bytes()));
        let rows=self.list(&format!("'{parent}' in parents and trashed=false and mimeType='application/vnd.google-apps.folder' and appProperties has {{ key='patterKey' and value='{key}' }}"))?;
        if let Some(id) = rows.first().and_then(|v| v["id"].as_str()) {
            return Ok(id.into());
        }
        let token = self.token()?;
        let v=value(send(self.http.post(self.url("/drive/v3/files")).bearer_auth(token).query(&[("fields","id")]).json(&json!({"name":name,"parents":[parent],"mimeType":"application/vnd.google-apps.folder","appProperties":{"patterKey":key}})))?)?;
        Ok(v["id"]
            .as_str()
            .ok_or("Google did not create the folder")?
            .into())
    }
    fn verify_remote(&mut self, id: &str, hash: &str, size: u64) -> Result<()> {
        valid_id(id)?;
        let token = self.token()?;
        let v = value(send(
            self.http
                .get(self.url(&format!("/drive/v3/files/{id}")))
                .bearer_auth(token)
                .query(&[("fields", "size,sha256Checksum,trashed")]),
        )?)?;
        if v["trashed"] == true
            || v["size"].as_str().and_then(|s| s.parse::<u64>().ok()) != Some(size)
        {
            return Err(
                "Google Drive verification failed. No complete backup was published.".into(),
            );
        }
        if let Some(remote_hash) = v["sha256Checksum"].as_str() {
            if remote_hash != hash {
                return Err(
                    "Google Drive verification failed. No complete backup was published.".into(),
                );
            }
        } else {
            // Drive may omit a checksum (for example, an empty file). Verify the actual bytes.
            let token = self.token()?;
            let response = checked(send(
                self.http
                    .get(self.url(&format!("/drive/v3/files/{id}")))
                    .bearer_auth(token)
                    .query(&[("alt", "media")]),
            )?)?;
            let mut response = response.take(size.saturating_add(1));
            let mut digest = Sha256::new();
            let mut count = 0u64;
            let mut buf = [0u8; 65536];
            loop {
                let n = response
                    .read(&mut buf)
                    .map_err(|_| "Remote verification was interrupted")?;
                if n == 0 {
                    break;
                }
                count += n as u64;
                digest.update(&buf[..n]);
            }
            if count != size || format!("{:x}", digest.finalize()) != hash {
                return Err(
                    "Google Drive verification failed. No complete backup was published.".into(),
                );
            }
        }
        Ok(())
    }
    pub fn upload(
        &mut self,
        parent: &str,
        name: &str,
        path: &Path,
        hash: &str,
        checkpoint: &Path,
    ) -> Result<String> {
        valid_id(parent)?;
        if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("Invalid upload hash".into());
        }
        let size = fs::metadata(path).map_err(io)?.len();
        let rows=self.list(&format!("'{parent}' in parents and trashed=false and appProperties has {{ key='patterSha256' and value='{hash}' }}"))?;
        for v in rows {
            if v["name"].as_str() != Some(name) {
                continue;
            }
            if let Some(id) = v["id"].as_str() {
                if self.verify_remote(id, hash, size).is_ok() {
                    return Ok(id.into());
                }
            }
        }
        if size == 0 {
            let token = self.token()?;
            let v=value(send(self.http.post(self.url("/drive/v3/files")).bearer_auth(token).query(&[("fields","id")]).json(&json!({"name":name,"mimeType":mime(name),"parents":[parent],"appProperties":{"patterSha256":hash}})))?)?;
            let id = v["id"].as_str().ok_or("Missing uploaded file ID")?;
            self.verify_remote(id, hash, size)?;
            return Ok(id.into());
        }
        let saved: Value = if checkpoint.exists() {
            read_json(checkpoint)?
        } else {
            Value::Null
        };
        let mut session = if saved["parent"] == parent
            && saved["hash"] == hash
            && saved["name"] == name
            && saved["size"] == size
        {
            saved["url"].as_str().unwrap_or("").to_owned()
        } else {
            String::new()
        };
        let mut offset = 0;
        if !session.is_empty() {
            self.validate_upload_url(&session)?;
            let token = self.token()?;
            let r = send(
                self.http
                    .put(&session)
                    .bearer_auth(token)
                    .header("Content-Length", "0")
                    .header("Content-Range", format!("bytes */{size}")),
            )?;
            if r.status().is_success() {
                let v = value(r)?;
                let id = v["id"].as_str().ok_or("Missing uploaded file ID")?;
                self.verify_remote(id, hash, size)?;
                return Ok(id.into());
            }
            if r.status().as_u16() == 308 {
                offset = resume_offset(&r, size)?;
            } else if matches!(r.status().as_u16(), 404 | 410) {
                session.clear();
            } else {
                checked(r)?;
            }
        }
        if session.is_empty() {
            let token = self.token()?;
            let props = if name.ends_with(".json")
                && path.file_name().and_then(|s| s.to_str()) == Some("remote-manifest.json")
            {
                json!({"patterSha256":hash,"patterSnapshot":"1"})
            } else {
                json!({"patterSha256":hash})
            };
            let r = checked(send(
                self.http
                    .post(self.url("/upload/drive/v3/files"))
                    .bearer_auth(token)
                    .query(&[
                        ("uploadType", "resumable"),
                        ("fields", "id,size,sha256Checksum"),
                    ])
                    .header("X-Upload-Content-Length", size)
                    .header("X-Upload-Content-Type", mime(name))
                    .json(&json!({"name":name,"parents":[parent],"appProperties":props})),
            )?)?;
            session = r
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or("Google did not start a resumable upload")?
                .into();
            self.validate_upload_url(&session)?;
            atomic_json(
                checkpoint,
                &json!({"url":session,"parent":parent,"hash":hash,"name":name,"size":size}),
            )?;
        }
        if offset == size {
            return Err("Google has not finalized this upload. Retry to verify it.".into());
        }
        let mut input = fs::File::open(path).map_err(io)?;
        loop {
            input.seek(SeekFrom::Start(offset)).map_err(io)?;
            let length = (size - offset).min(8 * 1024 * 1024);
            let mut bytes = vec![0u8; length as usize];
            input.read_exact(&mut bytes).map_err(io)?;
            let range = if size == 0 {
                "bytes */0".into()
            } else {
                format!("bytes {}-{}/{size}", offset, offset + length - 1)
            };
            let token = self.token()?;
            let r = send(
                self.http
                    .put(&session)
                    .bearer_auth(token)
                    .header("Content-Length", length)
                    .header("Content-Range", range)
                    .body(bytes),
            )?;
            if r.status().is_success() {
                let v = value(r)?;
                let id = v["id"].as_str().ok_or("Missing uploaded file ID")?;
                self.verify_remote(id, hash, size)?;
                return Ok(id.into());
            }
            if r.status().as_u16() != 308 {
                checked(r)?;
                return Err("Unexpected upload response".into());
            }
            let next = resume_offset(&r, size)?;
            if next <= offset || next > offset + length {
                return Err("Google returned an invalid upload offset. Retry the backup.".into());
            }
            offset = next;
            if offset == size {
                return Err("Google has not finalized this upload. Retry to verify it.".into());
            }
        }
    }
    pub fn snapshots(&mut self) -> Result<Value> {
        let files = self
            .list("trashed=false and appProperties has { key='patterSnapshot' and value='1' }")?;
        let mut files: Vec<Value> = files
            .into_iter()
            .map(|v| json!({"id":v["id"],"name":v["name"],"createdAt":v["createdTime"]}))
            .collect();
        files.sort_by(|a, b| b["createdAt"].as_str().cmp(&a["createdAt"].as_str()));
        Ok(json!(files))
    }
    fn download(&mut self, id: &str, dest: &Path, maximum: u64) -> Result<()> {
        valid_id(id)?;
        let token = self.token()?;
        let response = checked(send(
            self.http
                .get(self.url(&format!("/drive/v3/files/{id}")))
                .bearer_auth(token)
                .query(&[("alt", "media")]),
        )?)?;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)
            .map_err(io)?;
        let n = std::io::copy(&mut response.take(maximum.saturating_add(1)), &mut file)
            .map_err(|_| "Download interrupted. Retry restoring the backup.")?;
        if n > maximum {
            return Err("Downloaded file exceeds its declared size.".into());
        }
        file.sync_all().map_err(io)
    }
    pub fn download_snapshot(
        &mut self,
        id: &str,
        staging: &Path,
    ) -> Result<super::snapshot::Manifest> {
        // Only app-created completed snapshot files are eligible for restore.
        valid_id(id)?;
        let token = self.token()?;
        let meta = value(send(
            self.http
                .get(self.url(&format!("/drive/v3/files/{id}")))
                .bearer_auth(token)
                .query(&[("fields", "appProperties,sha256Checksum")]),
        )?)?;
        if meta["appProperties"]["patterSnapshot"] != "1" {
            return Err("Choose a Patter backup snapshot.".into());
        }
        self.download(id, &staging.join("manifest.json"), 32 * 1024 * 1024)?;
        if meta["sha256Checksum"].as_str()
            != Some(hash_file(&staging.join("manifest.json"))?.as_str())
        {
            return Err("The backup manifest failed verification.".into());
        }
        let manifest: super::snapshot::Manifest = read_json(&staging.join("manifest.json"))?;
        super::snapshot::validate(&manifest)?;
        for e in &manifest.entries {
            safe_relative(Path::new(&e.path))?;
            let dest = staging.join(&e.path);
            fs::create_dir_all(dest.parent().unwrap()).map_err(io)?;
            self.download(&e.drive_id, &dest, e.size)?;
            if hash_file(&dest)? != e.sha256 {
                return Err(format!("Restore verification failed for {}", e.path));
            }
        }
        Ok(manifest)
    }
}
fn valid_upload_url(s: &str) -> Result<()> {
    let u = reqwest::Url::parse(s).map_err(|_| "Invalid upload URL")?;
    if u.scheme() != "https"
        || u.host_str() != Some("www.googleapis.com")
        || u.port().is_some()
        || !u.path().starts_with("/upload/drive/v3/files")
        || !u.username().is_empty()
        || u.password().is_some()
    {
        return Err("Google returned an unexpected upload destination.".into());
    }
    Ok(())
}
fn resume_offset(r: &Response, size: u64) -> Result<u64> {
    let offset = if let Some(range) = r.headers().get("range") {
        range
            .to_str()
            .ok()
            .and_then(|s| s.strip_prefix("bytes=0-"))
            .and_then(|s| s.parse::<u64>().ok())
            .and_then(|n| n.checked_add(1))
            .ok_or("Invalid resumable upload range")?
    } else {
        0
    };
    if offset > size {
        return Err("Upload offset exceeds file size".into());
    }
    Ok(offset)
}
fn mime(name: &str) -> &'static str {
    if name.ends_with(".json") {
        "application/json"
    } else if name.ends_with(".md") {
        "text/markdown"
    } else if name.ends_with(".wav") {
        "audio/wav"
    } else if name.ends_with(".mp3") {
        "audio/mpeg"
    } else if name.ends_with(".ogg") {
        "audio/ogg"
    } else {
        "application/octet-stream"
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_credentials_rotate_without_changing_backup_identity() {
        let mut c: Config = serde_json::from_value(json!({"libraryId":"old-library", "deviceId":"old-device", "folderId":"old-folder", "connected":true, "enabled":true})).unwrap();
        assert_eq!(credential_account(&c), "old-library");
        let original = serde_json::to_value(&c).unwrap();
        assert!(rotate_credential_account(&mut c, |_| Err("locked".into())).is_err());
        assert_eq!(serde_json::to_value(&c).unwrap(), original);
        let mut saved = String::new();
        rotate_credential_account(&mut c, |account| {
            saved = account.into();
            Ok(())
        })
        .unwrap();
        assert_eq!(credential_account(&c), saved);
        assert_ne!(saved, "old-library");
        let mut rotated = serde_json::to_value(&c).unwrap();
        rotated["credentialAccount"] = json!("");
        assert_eq!(rotated, original);
        rotate_credential_account(&mut c, |account| {
            assert_ne!(account, saved);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn callback_accepts_a_request_split_across_packets() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut browser = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut accepted, _) = listener.accept().unwrap();
        browser.write_all(b"GET /callback?state=fixture").unwrap();
        let thread = std::thread::spawn(move || {
            let line = read_callback_line(&mut accepted).unwrap();
            assert_eq!(callback(&line, "fixture").unwrap().unwrap(), "test-code");
        });
        std::thread::sleep(Duration::from_millis(50));
        browser
            .write_all(b"&code=test-code HTTP/1.1\r\n\r\n")
            .unwrap();
        thread.join().unwrap();
    }
    #[test]
    fn oauth_callback_rejects_wrong_state_and_unsafe_upload_targets() {
        assert!(callback("GET /callback?state=bad&code=secret HTTP/1.1", "good").is_none());
        assert_eq!(
            callback("GET /callback?state=good&code=test HTTP/1.1", "good")
                .unwrap()
                .unwrap(),
            "test"
        );
        assert!(
            callback("GET /callback?state=good&error=denied HTTP/1.1", "good")
                .unwrap()
                .is_err()
        );
        assert!(valid_upload_url("https://evil.example/upload/drive/v3/files").is_err());
        assert!(valid_upload_url(
            "https://www.googleapis.com/upload/drive/v3/files?upload_id=fixture"
        )
        .is_ok());
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    struct Step {
        method: &'static str,
        path: &'static str,
        range: Option<String>,
        body: Option<Vec<u8>>,
        code: u16,
        headers: Vec<(String, String)>,
        response: Value,
    }
    fn step(method: &'static str, path: &'static str, response: Value) -> Step {
        Step {
            method,
            path,
            range: None,
            body: None,
            code: 200,
            headers: vec![],
            response,
        }
    }
    fn server(listener: TcpListener, steps: Vec<Step>) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            listener.set_nonblocking(true).unwrap();
            for step in steps {
                let deadline = Instant::now() + Duration::from_secs(15);
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(s) => break s,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "Missing mock request");
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                stream.set_nonblocking(false).unwrap();
                let mut reader = BufReader::new(&mut stream);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert!(
                    line.starts_with(&format!("{} {}", step.method, step.path)),
                    "Unexpected request: {line}"
                );
                let mut headers = std::collections::HashMap::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    let (k, v) = line.split_once(':').unwrap();
                    headers.insert(k.to_ascii_lowercase(), v.trim().to_owned());
                }
                assert_eq!(
                    headers.get("authorization").map(String::as_str),
                    Some("Bearer fixture-access")
                );
                if let Some(range) = step.range {
                    assert_eq!(headers.get("content-range"), Some(&range));
                }
                let length = headers
                    .get("content-length")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let mut body = vec![0u8; length];
                reader.read_exact(&mut body).unwrap();
                if let Some(expected) = step.body {
                    assert_eq!(body, expected);
                }
                let response = if step.response.is_null() {
                    String::new()
                } else {
                    step.response.to_string()
                };
                let extra = step
                    .headers
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}\r\n"))
                    .collect::<String>();
                write!(stream,"HTTP/1.1 {} Response\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",step.code,response.len(),extra,response).unwrap();
            }
        })
    }
    fn drive(base: String) -> Drive {
        Drive {
            base,
            http: client().unwrap(),
            secret: Credentials {
                client_id: "fixture".into(),
                client_secret: "fixture".into(),
                refresh_token: "fixture".into(),
            },
            access: "fixture-access".into(),
            expires: Instant::now() + Duration::from_secs(300),
        }
    }
    #[test]
    fn interrupted_upload_resumes_and_repeat_upload_reuses_verified_object() {
        let root = std::env::temp_dir().join(format!("patter-upload-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("audio.wav");
        let bytes = vec![42u8; 8 * 1024 * 1024 + 19];
        fs::write(&path, &bytes).unwrap();
        let hash = hash_file(&path).unwrap();
        let size = bytes.len();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let session = format!("{base}/upload/drive/v3/files?upload_id=fixture");
        let mut start = step("POST", "/upload/drive/v3/files", json!({}));
        start.headers.push(("Location".into(), session));
        let mut chunk = step("PUT", "/upload/drive/v3/files", json!({}));
        chunk.code = 308;
        chunk.range = Some(format!("bytes 0-8388607/{size}"));
        chunk.body = Some(bytes[..8388608].to_vec());
        chunk
            .headers
            .push(("Range".into(), "bytes=0-8388607".into()));
        let mut failure = step("PUT", "/upload/drive/v3/files", json!({}));
        failure.code = 503;
        let mut probe = step("PUT", "/upload/drive/v3/files", json!({}));
        probe.code = 308;
        probe.range = Some(format!("bytes */{size}"));
        probe
            .headers
            .push(("Range".into(), "bytes=0-8388607".into()));
        let mut final_chunk = step(
            "PUT",
            "/upload/drive/v3/files",
            json!({"id":"recording-id"}),
        );
        final_chunk.range = Some(format!("bytes 8388608-{}/{size}", size - 1));
        final_chunk.body = Some(bytes[8388608..].to_vec());
        let metadata = json!({"size":size.to_string(),"sha256Checksum":hash,"trashed":false});
        let thread = server(
            listener,
            vec![
                step("GET", "/drive/v3/files", json!({"files":[]})),
                start,
                chunk,
                failure,
                step("GET", "/drive/v3/files", json!({"files":[]})),
                probe,
                final_chunk,
                step("GET", "/drive/v3/files/recording-id", metadata.clone()),
                step(
                    "GET",
                    "/drive/v3/files",
                    json!({"files":[{"id":"recording-id","name":"audio.wav"}]}),
                ),
                step("GET", "/drive/v3/files/recording-id", metadata),
            ],
        );
        let checkpoint = root.join("resume.json");
        let mut first = drive(base.clone());
        assert!(first
            .upload("parent", "audio.wav", &path, &hash, &checkpoint)
            .unwrap_err()
            .contains("503"));
        let mut restarted = drive(base);
        assert_eq!(
            restarted
                .upload("parent", "audio.wav", &path, &hash, &checkpoint)
                .unwrap(),
            "recording-id"
        );
        assert_eq!(
            restarted
                .upload("parent", "audio.wav", &path, &hash, &checkpoint)
                .unwrap(),
            "recording-id"
        );
        thread.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn remote_checksum_mismatch_cannot_report_success() {
        let root = std::env::temp_dir().join(format!("patter-upload-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("notes.md");
        fs::write(&path, b"notes").unwrap();
        let hash = hash_file(&path).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let mut start = step("POST", "/upload/drive/v3/files", json!({}));
        start.headers.push((
            "Location".into(),
            format!("{base}/upload/drive/v3/files?upload_id=fixture"),
        ));
        let thread = server(
            listener,
            vec![
                step("GET", "/drive/v3/files", json!({"files":[]})),
                start,
                step("PUT", "/upload/drive/v3/files", json!({"id":"bad"})),
                step(
                    "GET",
                    "/drive/v3/files/bad",
                    json!({"size":"5","sha256Checksum":"bad","trashed":false}),
                ),
            ],
        );
        assert!(drive(base)
            .upload("parent", "notes.md", &path, &hash, &root.join("checkpoint"))
            .unwrap_err()
            .contains("verification failed"));
        thread.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn empty_exports_keep_distinct_names_and_verify_download_when_checksum_is_absent() {
        let root =
            std::env::temp_dir().join(format!("patter-empty-upload-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("summary.md");
        fs::write(&path, b"").unwrap();
        let hash = hash_file(&path).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let thread = server(
            listener,
            vec![
                step(
                    "GET",
                    "/drive/v3/files",
                    json!({"files":[{"id":"notes","name":"notes.md"}]}),
                ),
                step("POST", "/drive/v3/files", json!({"id":"summary"})),
                step(
                    "GET",
                    "/drive/v3/files/summary",
                    json!({"size":"0","trashed":false}),
                ),
                step("GET", "/drive/v3/files/summary", Value::Null),
            ],
        );
        assert_eq!(
            drive(base)
                .upload(
                    "parent",
                    "summary.md",
                    &path,
                    &hash,
                    &root.join("checkpoint")
                )
                .unwrap(),
            "summary"
        );
        thread.join().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
