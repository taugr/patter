use crate::{store::Result, transcription_models};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub fn selected_model(preferences: &Value) -> &str {
    preferences["transcriptionModel"]
        .as_str()
        .unwrap_or_else(|| {
            if preferences["whisperModel"]
                .as_str()
                .is_some_and(|p| !p.is_empty())
            {
                "whisper-file"
            } else {
                "parakeet-v3"
            }
        })
}
pub fn transcribe(
    root: &Path,
    meeting: &Value,
    preferences: &Value,
    parakeet: &Path,
) -> Result<Vec<Value>> {
    let id = meeting["id"].as_str().ok_or("Missing conversation id")?;
    let recordings = meeting["recordings"]
        .as_array()
        .filter(|r| !r.is_empty())
        .ok_or("Import or record some audio first.")?;
    let allowed = root
        .join("recordings")
        .join(id)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let mut inputs = Vec::new();
    for r in recordings {
        let path = PathBuf::from(r["path"].as_str().ok_or("Missing recording path")?)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !path.starts_with(&allowed) {
            return Err("Recording path is outside this conversation.".into());
        }
        inputs.push(json!({"path":path,"offset":r["offset"].as_f64().filter(|o| o.is_finite() && *o >= 0.0).unwrap_or(0.0),"track":r["track"].as_str().unwrap_or("Audio")}));
    }
    let selected = selected_model(preferences);
    let model = if selected == "whisper-file" {
        PathBuf::from(
            preferences["whisperModel"]
                .as_str()
                .filter(|p| Path::new(p).is_file())
                .ok_or("Choose a local Whisper .bin model in Settings first.")?,
        )
    } else {
        let model = transcription_models::model(selected)?;
        let folder = model.verify(root)?;
        if selected == "parakeet-v3" {
            folder
        } else {
            folder.join(&model.files[0].path)
        }
    };
    let working = root
        .join("processing")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&working).map_err(|e| e.to_string())?;
    let mut segments = if selected == "parakeet-v3" {
        let request = working.join("recordings.json");
        let output = working.join("parakeet-transcript.json");
        std::fs::write(
            &request,
            serde_json::to_vec(&inputs).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let result = Command::new(parakeet)
            .arg(model)
            .arg(request)
            .arg(&output)
            .output()
            .map_err(|e| format!("Could not start Parakeet: {e}"))?;
        if !result.status.success() {
            return Err(format!(
                "Parakeet could not transcribe this recording: {}",
                String::from_utf8_lossy(&result.stderr)
                    .chars()
                    .take(1200)
                    .collect::<String>()
            ));
        }
        let values: Vec<Value> =
            serde_json::from_slice(&std::fs::read(output).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if !values.iter().all(|s| {
            s["start"]
                .as_f64()
                .is_some_and(|n| n.is_finite() && n >= 0.0)
                && s["text"].is_string()
                && s["speaker"].is_string()
        }) {
            return Err(
                "Parakeet returned an invalid transcript; saved content is unchanged.".into(),
            );
        }
        values
    } else {
        whisper(&model, &inputs, &working)?
    };
    segments.sort_by(|a, b| {
        a["start"]
            .as_f64()
            .partial_cmp(&b["start"].as_f64())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(segments)
}
fn whisper(model: &Path, recordings: &[Value], working: &Path) -> Result<Vec<Value>> {
    let context = whisper_rs::WhisperContext::new_with_params(
        model.to_str().ok_or("Invalid model path")?,
        whisper_rs::WhisperContextParameters::default(),
    )
    .map_err(|e| format!("Whisper could not load the selected model: {e}"))?;
    let mut segments = Vec::new();
    for (i, r) in recordings.iter().enumerate() {
        let wave = working.join(format!("{i}.wav"));
        let result = Command::new("/usr/bin/afconvert")
            .arg(r["path"].as_str().unwrap())
            .arg(&wave)
            .args(["-f", "WAVE", "-d", "LEI16@16000", "-c", "1"])
            .output()
            .map_err(|e| e.to_string())?;
        if !result.status.success() {
            return Err(format!(
                "Could not prepare audio: {}",
                String::from_utf8_lossy(&result.stderr)
            ));
        }
        let mut reader = hound::WavReader::open(&wave).map_err(|e| e.to_string())?;
        let samples = reader
            .samples::<i16>()
            .map(|s| s.map(|x| x as f32 / 32768.0))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let mut state = context.create_state().map_err(|e| e.to_string())?;
        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(None);
        params.set_translate(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_n_threads(4);
        state.full(params, &samples).map_err(|e| e.to_string())?;
        let offset = r["offset"].as_f64().unwrap_or(0.0);
        for s in state.as_iter() {
            segments.push(json!({"start":offset+s.start_timestamp() as f64/100.0,"text":s.to_str_lossy().map_err(|e| e.to_string())?.trim(),"speaker":r["track"]}));
        }
    }
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_whisper_selection_is_preserved() {
        assert_eq!(
            selected_model(&json!({"whisperModel":"/models/old.bin"})),
            "whisper-file"
        );
        assert_eq!(selected_model(&json!({})), "parakeet-v3");
        assert_eq!(
            selected_model(
                &json!({"whisperModel":"/models/old.bin","transcriptionModel":"parakeet-v3"})
            ),
            "parakeet-v3"
        );
    }
}
