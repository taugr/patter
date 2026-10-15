# Synthetic recording smoke test

`file-capture.swift` feeds a caller-supplied synthetic speech file into production
`AudioChunks`, waits for the same `stop` pipe command as the native recorder, and
finalizes the chunks. It does not create `Recorder`, microphone taps, system-audio
taps, playback or permission prompts. Its microphone-access stub always throws.

The ignored Rust test in `src-tauri/src/recording_smoke_tests.rs` exercises the
production stop/completion function, WAV conversion, automatic processing,
Parakeet transcription, loopback summary generation, SQLite history and reopening.
It also checks that completed work is not repeated and that failed/empty captures
keep audio and the previous summary without queueing a new generation.

Run only with a new, explicitly isolated directory and already-installed models.
This is deliberately excluded from normal CI because it performs real local
inference. It never downloads models; do not use the older `transcription-smoke`
example, which has a download step, for this check.

1. Create a new test directory, with `models/parakeet-v3/` copied from an installed
   Patter model cache. APFS copy-on-write copies (`cp -cR`) avoid duplicating weights.
   Never point the test root at the real Patter library. The root must have no
   `patter.sqlite3` and must contain `.synthetic-test-library` with exactly
   `synthetic-only` plus a newline.
2. Render a short synthetic script directly to a file using an installed voice,
   for example `say -v Samantha -o /tmp/synthetic.aiff -f /tmp/synthetic.txt`.
   Do not play it through speakers or capture unrelated live sound.
3. Compile the file-fed helper (from the repository root):

   ```sh
   swiftc -O -parse-as-library -target arm64-apple-macosx15.0 \
     -module-cache-path /tmp/patter-smoke-swift-cache \
     src-tauri/native/AudioCapture.swift test-fixtures/file-capture.swift \
     -o /tmp/patter-file-capture
   ```

4. Set all five environment variables explicitly and run:

   ```sh
   PATTER_RECORDING_SMOKE_ROOT=/tmp/new-isolated-library \
   PATTER_RECORDING_SMOKE_AUDIO=/tmp/synthetic.aiff \
   PATTER_RECORDING_SMOKE_HELPER=/tmp/patter-file-capture \
   PATTER_RECORDING_SMOKE_MODEL=already-installed-model-id \
   PATTER_RECORDING_SMOKE_ENDPOINT=http://127.0.0.1:1234/v1 \
   cargo test --offline --locked --manifest-path src-tauri/Cargo.toml \
     recording_smoke_tests::file_audio_completes_transcribes_summarizes_and_reopens_with_real_local_models \
     -- --ignored --nocapture
   ```

The test preserves its isolated library and writes `successful-meeting.json`,
`final-meeting.json` and `checks.json` there. Review the transcript and summary for
factual accuracy and supported actions; schema validation alone cannot establish
that. Preserve the original local-model loaded state, unloading only a model
loaded solely for the test when finished.

This verifies the file-fed pipeline, not genuine device capture or native UI
permission handling. Real microphone/system audio needs a separate supervised
check in a quiet setting with no unrelated calls or media running.
