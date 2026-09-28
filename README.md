# Patter

<p align="center">
  <img src="./public/patter-icon.svg" alt="Patter app icon" width="140" />
  <br />
  <a href="https://github.com/taugr/patter/releases/latest">
    <img src="https://img.shields.io/github/v/release/taugr/patter?color=1b7180" alt="Latest release" />
  </a>
  <a href="https://github.com/taugr/patter/actions/workflows/ci.yml">
    <img src="https://github.com/taugr/patter/actions/workflows/ci.yml/badge.svg" alt="Build and tests" />
  </a>
  <img src="https://img.shields.io/badge/macOS-15%2B-282723?logo=apple&logoColor=white" alt="macOS 15 or later" />
  <img src="https://img.shields.io/badge/Tauri-2-24c8d8?logo=tauri&logoColor=white" alt="Built with Tauri 2" />
  <img src="https://img.shields.io/badge/local--first-1b7180" alt="Local-first" />
  <br />
  A personal meeting notebook for Mac. Record conversations, transcribe locally,
  and keep your notes, summaries and audio together.
</p>

<p align="center">
  <a href="https://github.com/taugr/patter/releases/latest">Download for Mac</a> ·
  <a href="./INSTALL.md">Installation guide</a> ·
  <a href="#first-setup">First setup</a> ·
  <a href="#run">Development</a>
</p>

## Overview

A personal, local-first meeting notebook for Apple Silicon Macs running macOS 15 or later. Built with Tauri 2, Rust, SQLite, Swift audio/calendar and Parakeet helpers, and React + Vite. No hosted backend, account system, analytics, Electron runtime, or bundled model weights.

Patter is in active personal development. **v0.4.0** adds Parakeet model downloads, Anarlog import and Google Drive backup. Native audio capture, Calendar permissions and summary-model inference still need a supervised setup and reliability pass before relying on Patter for important meetings.

## Install and updates

Download the [latest Mac installer](https://github.com/taugr/patter/releases/latest). Drag Patter into Applications. Later, use Settings → Updates → Install and restart. Read [installation, release and recovery instructions](INSTALL.md), including the ad-hoc signing caveat. No Apple Developer membership or notarization is used.

## Run

Requires Node 24.14, pnpm (pinned in package.json), Rust 1.96, and Swift 6.2+ / Xcode 26 command-line tools (Swift/C++). CI uses Xcode 26.3. The checked-in Cargo/CMake configuration targets a portable M1 CPU baseline while keeping Metal acceleration enabled.

```sh
pnpm install
pnpm dev                 # browser design preview, with labelled examples
pnpm desktop             # native development app; stop a separate pnpm dev first
pnpm desktop:build       # standalone local Mac application
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
```

Application: `src-tauri/target/release/bundle/macos/Patter.app`. Model weights are downloaded separately from the application. It is not a signed/notarized distribution release. The native app starts with an empty library; the browser examples never populate it. One clearly labelled welcome/test note and synthetic audio were added during native QA on this Mac.

## Features

- Cream, ink and teal split-view notebook with Fraunces headings, Nunito Sans body text, the vector Patter mark and Phosphor icons.
- Editable conversation titles/notes, summaries, decisions, action checkboxes, transcripts, search, Today filter and archive/restore.
- General meeting, One-to-one, Interview, Brainstorm and Custom summary templates, with editable instructions, a default preference and per-conversation choices.
- Immutable SQLite versions; restoring creates a new version. Existing audio attachments cannot be detached or repointed by ordinary saves. No delete/retention-cleanup commands.
- Audio import with a durable copy, playback, timestamp seeking, speed control, and consecutive chunk playback within the selected track.
- Swift ScreenCaptureKit bridge for microphone and computer audio; approximately five-second CAF chunks, unique capture-session filenames, startup recovery of surviving chunks, and WAV playback derivatives. No screen video is stored.
- Local Parakeet v3 inference through FluidAudio/CoreML and Whisper through whisper.cpp/Metal. In-app model downloads are pinned by revision, size and SHA-256; partial downloads can resume. Existing user-selected Whisper `.bin` files remain supported. Summaries use a separate local OpenAI-compatible server restricted to loopback, with proxy forwarding and redirects disabled.
- EventKit calendar reading, including Google calendars already synced to macOS; upcoming events can create or reopen linked notes. No Google OAuth application is needed for this route.
- JSON conversation export and native library backup (SQLite online backup plus original recording files). Browser preview data uses IndexedDB and is separate from the permanent Mac library.

## First setup

**Calendar:** Add the Google account in macOS Internet Accounts and enable Calendars. Confirm the events appear in Apple Calendar, then use Patter Settings → Connect Mac calendar and grant access. macOS requires full calendar access for EventKit reads; Patter's implementation only reads. Events refresh on launch or when connecting again. Direct Google OAuth, calendar selection and background refresh are future work.

**Summaries:** Start a model in LM Studio's local server, then use `http://127.0.0.1:1234/v1`. For an existing Ollama server use `http://127.0.0.1:11434/v1`. Choose Find models, enter/select a model, and Save. The endpoint/model are preferences, not credentials. No cloud fallback is implemented.

**Templates:** In Settings → Summary templates, choose a default and edit its instructions. Each template retains its own edits; Reset restores its built-in instructions. In a conversation's Overview, expand Summary template to override the default or add extra instructions. Changes apply on the next generation, using the same summary, decisions and next steps layout. Long-transcript processing also uses the selected instructions. Generated versions retain the exact instructions, template name, model and generation time; changing a template does not rewrite past summaries. Existing libraries default to General meeting without a database migration.

**Transcription:** In Settings → Transcription, select Parakeet v3 (632 MB), Whisper Base (148 MB), or Whisper Small (488 MB), choose Download model, then Save. Downloads come from Hugging Face; each file is verified before the model is marked ready. Cancellation/interruption keeps partial files for retry, and Verify / repair checks installed files. Once installed, transcription stays offline. Parakeet v3 supports English and 24 other European languages; Whisper supports broader language coverage. The manual Whisper file option accepts compatible whisper.cpp GGML `.bin` files, and older configured paths are preserved. Import or record audio, open Transcript, and choose Transcribe recording. Audio track labels (Microphone / Computer audio) are not speaker diarization. Captured chunks are still transcribed individually; improving long-call context remains future work.

**Recording:** Record → Record microphone & computer audio. macOS may request microphone and screen/system-audio capture permissions. Capture was compiled but not activated during QA. Test permissions and playback on a short disposable conversation first.

## Permanent library

On this Mac: `~/Library/Application Support/gr.tau.patter/`.

- `patter.sqlite3`: current conversations, immutable versions and preferences.
- `recordings/<conversation-id>/`: original imported files and captured chunks; playback derivatives may also live here.
- `processing/`: retained conversion files and Parakeet outputs from transcription attempts.
- `models/`: versioned model downloads and partial files. Library backups include the database and original recordings; downloadable models can be downloaded again.

Patter never automatically purges these files. Archive changes visibility. Each committed edit/regeneration is versioned; draft keystrokes are coalesced with a 600 ms debounce, and normal closing/quit waits for pending saves. Forced termination can lose an unsaved draft or the currently open capture chunk. App retention cannot protect against disk failure or manual filesystem deletion: use library backups.

For manual library restore, quit Patter and preserve the current library separately before restoring. Recording paths are absolute, so moving a manual backup to another account/path requires migration. Google Drive restore validates backups, remaps recording paths and retains a safety copy of the previous library; see [backup and restore instructions](docs/google-drive-backup.md). Real Drive backup/restore and LaunchAgent catch-up with Patter closed have passed. After an ad-hoc update, Patter may require Drive reconnection using the original Google setup JSON. That recovery path and resumed background backup have been verified; see the backup guide for the evidence and remaining checks. Low-disk, long-call, sleep/wake and device-change checks are also outstanding.

## Verified here

- Production TypeScript/Vite build and Tauri `.app` packaging.
- Frontend storage/search/template tests and Rust storage/model/template/update-safety tests, including a mock local-model server that checks template guidance through long-transcript processing. Backup fixtures cover snapshot/restore integrity, scheduling and upload behavior; see [validation coverage and remaining checks](docs/google-drive-backup.md#validation-completed-and-still-needed). These checks do not verify real model output quality or Google authorization.
- Browser flows: note editing, retained versions, restore, archive/restore, text search, action checkbox persistence, tabs, sample audio play/seek, desktop and compact layouts. No browser console errors in the final inspection.
- Native app launch, creating/editing a note, Cmd-Q and reopening with notes intact, importing synthetic WAV audio, and native playback.
- Real Parakeet v3 and Whisper Base downloads and inference on the synthetic 15-second sample, preserving audio bytes and track offsets. Packaged Mac app: Parakeet download, saved selection, successful transcription, and cancellation/retry of Whisper Small. Long meetings and additional hardware still need validation.

Not yet verified: live capture, Google/EventKit permission flow, live-meeting transcription quality or LLM output, full backup/restore round trip, long-session recovery/performance. No real microphone/system capture or personal calendar reading was performed.

## Attribution and licensing

Patter is a fresh implementation inspired by [Anarlog](https://github.com/fastrepl/anarlog); no upstream application source was copied. The bundled example conversation and narration are synthetic. The vector logo was created for Patter. Fonts are Fraunces and Nunito Sans, with Phosphor icons.

No source-code license has been selected yet; public visibility does not grant a general license to this application's original code. Third-party components retain their own licenses; see `THIRD_PARTY_NOTICES.txt`.

## Parakeet implementation

The small `patter-parakeet` executable links pinned [FluidAudio 0.17.1](https://github.com/FluidInference/FluidAudio/tree/v0.17.1) and uses its offline `loadLocal` API. Python and a model server are not required. Optional NeMo text-processing is disabled. `src-tauri/transcription-models.json` pins the Hugging Face repositories, revisions, file sizes and SHA-256 values. The Parakeet weights are from [FluidInference's CoreML conversion](https://huggingface.co/FluidInference/parakeet-tdt-0.6b-v3-coreml) of [NVIDIA Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), under CC-BY-4.0; Whisper weights are MIT-licensed. Model files are not bundled or uploaded with recordings.

An opt-in smoke test downloads a model into a separate test directory, transcribes the supplied audio, checks offsets and verifies that the original audio is unchanged:

```sh
cargo run --locked --manifest-path src-tauri/Cargo.toml --example transcription-smoke -- /tmp/patter-transcription-proof parakeet-v3 public/sample-conversation.wav
```

**Moving from Anarlog 1.0.27:** Settings → Import from Anarlog previews and imports a copied Content/sessions folder, including notes, summaries, transcripts and retained audio. Original session files are preserved, repeat imports skip unchanged conversations, and existing Patter edits are never overwritten. See [the transfer instructions and format limits](INSTALL.md#import-an-anarlog-1027-library). Available from v0.4.0.

### Google Drive backup

Connect your own Google Drive in Settings, choose a nightly time, or back up manually. Patter uploads new/changed recordings, readable transcripts/summaries, complete version history and nonsecret configuration. Previous backups are kept. A verified restore remaps paths for another Mac and preserves the current library in a safety folder. [Setup, coverage and validation limits](docs/google-drive-backup.md).
