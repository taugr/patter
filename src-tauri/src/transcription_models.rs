use crate::store::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Deserialize, Serialize)]
pub struct ModelFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub description: String,
    pub repo: String,
    pub revision: String,
    pub license: String,
    pub files: Vec<ModelFile>,
}
impl Model {
    pub fn directory(&self, root: &Path) -> PathBuf {
        root.join("models").join(&self.id).join(&self.revision)
    }
    pub fn size(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }
    pub fn installed(&self, root: &Path) -> bool {
        let dir = self.directory(root);
        fs::read_to_string(dir.join(".complete")).is_ok_and(|s| s == self.revision)
            && self.files.iter().all(|f| {
                fs::metadata(dir.join(&f.path)).is_ok_and(|m| m.is_file() && m.len() == f.size)
            })
    }
    pub fn verify(&self, root: &Path) -> Result<PathBuf> {
        let directory = self.directory(root);
        if !self.installed(root) {
            return Err(format!(
                "Download {} in Settings → Transcription first.",
                self.name
            ));
        }
        for file in &self.files {
            if !valid_file(&directory.join(&file.path), file)? {
                return Err(format!(
                    "{} needs repair. Download it again in Settings → Transcription.",
                    self.name
                ));
            }
        }
        Ok(directory)
    }
}
pub fn catalog() -> Vec<Model> {
    serde_json::from_str(include_str!("../transcription-models.json"))
        .expect("valid bundled model catalog")
}
pub fn model(id: &str) -> Result<Model> {
    catalog()
        .into_iter()
        .find(|m| m.id == id)
        .ok_or("Unknown transcription model.".into())
}
fn valid_file(path: &Path, expected: &ModelFile) -> Result<bool> {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.to_string()),
    };
    if file.metadata().map_err(|e| e.to_string())?.len() != expected.size {
        return Ok(false);
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 128 * 1024];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()) == expected.sha256)
}
#[derive(Clone, Serialize)]
pub struct Progress {
    pub id: String,
    pub downloaded: u64,
    pub total: u64,
    pub phase: String,
}
#[derive(Default, Clone)]
pub struct Downloads {
    progress: Arc<Mutex<Option<Progress>>>,
    cancelled: Arc<AtomicBool>,
}
struct Lease(Downloads);
impl Drop for Lease {
    fn drop(&mut self) {
        if let Ok(mut p) = self.0.progress.lock() {
            *p = None;
        }
    }
}
impl Downloads {
    pub fn progress(&self) -> Result<Option<Progress>> {
        Ok(self
            .progress
            .lock()
            .map_err(|_| "Download state unavailable")?
            .clone())
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
    fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::SeqCst) {
            Err("Download cancelled. Partial files are kept for retry.".into())
        } else {
            Ok(())
        }
    }
    fn begin(&self, model: &Model) -> Result<Lease> {
        let mut p = self
            .progress
            .lock()
            .map_err(|_| "Download state unavailable")?;
        if p.is_some() {
            return Err("A model is already downloading.".into());
        }
        self.cancelled.store(false, Ordering::SeqCst);
        *p = Some(Progress {
            id: model.id.clone(),
            downloaded: 0,
            total: model.size(),
            phase: "Preparing download".into(),
        });
        Ok(Lease(self.clone()))
    }
    fn update(&self, downloaded: u64, phase: &str) {
        if let Ok(mut p) = self.progress.lock() {
            if let Some(p) = p.as_mut() {
                p.downloaded = downloaded;
                p.phase = phase.into();
            }
        }
    }
}

/// Only pinned, bundled files are downloadable. Recording and database paths are never touched.
pub async fn download(root: &Path, model: &Model, state: &Downloads) -> Result<()> {
    let _lease = state.begin(model)?;
    let client = reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let directory = model.directory(root);
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let mut completed = 0;
    for entry in &model.files {
        state.check()?;
        // Defense against accidental unsafe paths in future catalog edits.
        if !Path::new(&entry.path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err("Invalid model file path.".into());
        }
        let dest = directory.join(&entry.path);
        if valid_file(&dest, entry)? {
            completed += entry.size;
            state.update(completed, "Downloading");
            continue;
        }
        fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
        let partial = dest.with_file_name(format!(
            "{}.part",
            dest.file_name().unwrap().to_string_lossy()
        ));
        let url = format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            model.repo, model.revision, entry.path
        );
        download_file(&client, &url, &partial, entry, state, completed).await?;
        state.update(completed + entry.size, "Verifying download");
        state.check()?;
        if !valid_file(&partial, entry)? {
            // Keep failed bytes for inspection, while a retry gets a fresh partial file.
            let failed = partial.with_extension(format!("failed-{}", uuid::Uuid::new_v4()));
            fs::rename(&partial, failed).map_err(|e| e.to_string())?;
            return Err(
                "The downloaded model failed verification. Please retry the download.".into(),
            );
        }
        fs::rename(&partial, &dest).map_err(|e| e.to_string())?;
        completed += entry.size;
    }
    state.check()?;
    let marker = directory.join(".complete.tmp");
    let mut file = File::create(&marker).map_err(|e| e.to_string())?;
    file.write_all(model.revision.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(marker, directory.join(".complete")).map_err(|e| e.to_string())?;
    Ok(())
}
async fn download_file(
    client: &reqwest::Client,
    url: &str,
    partial: &Path,
    entry: &ModelFile,
    state: &Downloads,
    completed: u64,
) -> Result<()> {
    let mut existing = fs::metadata(partial).map(|m| m.len()).unwrap_or(0);
    if existing == entry.size {
        return Ok(());
    }
    if existing > entry.size {
        existing = 0;
    }
    let mut request = client.get(url);
    if existing > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={existing}-"));
    }
    let mut response = request
        .send()
        .await
        .map_err(|e| format!("Download interrupted; retry to continue: {e}"))?;
    state.check()?;
    if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        let range = response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !range.starts_with(&format!("bytes {existing}-"))
            || !range.ends_with(&format!("/{}", entry.size))
        {
            return Err("Invalid download range; please retry.".into());
        }
    } else if response.status() == reqwest::StatusCode::OK {
        existing = 0;
    } else {
        return Err(format!(
            "Model download returned {}. Please retry.",
            response.status()
        ));
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(existing == 0)
        .append(existing > 0)
        .open(partial)
        .map_err(|e| e.to_string())?;
    let mut received = existing;
    let mut updated = Instant::now();
    state.update(completed + received, "Downloading");
    while let Some(bytes) = response
        .chunk()
        .await
        .map_err(|e| format!("Download interrupted; retry to continue: {e}"))?
    {
        state.check()?;
        if received + bytes.len() as u64 > entry.size {
            return Err("Model download exceeded its expected size.".into());
        }
        file.write_all(&bytes)
            .map_err(|e| format!("Could not save model (check free disk space): {e}"))?;
        received += bytes.len() as u64;
        if updated.elapsed() > Duration::from_millis(150) {
            state.update(completed + received, "Downloading");
            updated = Instant::now();
        }
    }
    file.sync_all().map_err(|e| e.to_string())?;
    if received != entry.size {
        return Err("Download incomplete. Retry to continue.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        net::TcpListener,
    };
    fn entry() -> ModelFile {
        ModelFile {
            path: "model.bin".into(),
            size: 6,
            sha256: format!("{:x}", Sha256::digest(b"abcdef")),
        }
    }
    #[test]
    fn catalog_is_pinned_and_completion_requires_all_files() {
        for model in catalog() {
            assert_eq!(model.revision.len(), 40);
            assert!(!model.files.is_empty());
            assert!(model.files.iter().all(|f| f.sha256.len() == 64
                && f.size > 0
                && Path::new(&f.path)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))));
        }
        let root = std::env::temp_dir().join(format!("patter-model-test-{}", uuid::Uuid::new_v4()));
        let model = Model {
            id: "test".into(),
            name: "test".into(),
            description: "".into(),
            repo: "test".into(),
            revision: "pinned".into(),
            license: "MIT".into(),
            files: vec![entry()],
        };
        let dir = model.directory(&root);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".complete"), "pinned").unwrap();
        assert!(!model.installed(&root));
        fs::write(dir.join("model.bin"), "abcdef").unwrap();
        assert!(model.verify(&root).is_ok());
        fs::write(dir.join("model.bin"), "tamper").unwrap();
        assert!(model.verify(&root).is_err());
        let downloads = Downloads::default();
        let lease = downloads.begin(&model).unwrap();
        assert!(downloads.begin(&model).is_err());
        downloads.cancel();
        assert!(downloads.check().is_err());
        drop(lease);
        assert!(downloads.progress().unwrap().is_none());
        let _retry = downloads.begin(&model).unwrap();
        assert!(downloads.check().is_ok());
    }
    fn range_case(resumes: bool) {
        let root = std::env::temp_dir().join(format!("patter-range-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let partial = root.join("model.part");
        fs::write(&partial, "abc").unwrap();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/model", server.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = server.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                headers.push_str(&line);
            }
            assert!(headers.to_lowercase().contains("range: bytes=3-"));
            let response = if resumes {
                "HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 3-5/6\r\nConnection: close\r\n\r\ndef"
            } else {
                "HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabcdef"
            };
            stream.write_all(response.as_bytes()).unwrap();
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        tauri::async_runtime::block_on(download_file(
            &client,
            &url,
            &partial,
            &entry(),
            &Downloads::default(),
            0,
        ))
        .unwrap();
        thread.join().unwrap();
        assert!(valid_file(&partial, &entry()).unwrap());
    }
    #[test]
    fn resumes_a_partial_download() {
        range_case(true);
    }
    #[test]
    fn restarts_when_the_server_ignores_range() {
        range_case(false);
    }
}
