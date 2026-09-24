# Patter

A personal, local-first meeting notebook for Apple Silicon Macs running macOS 15 or later. Built with Tauri 2, Rust, SQLite, a small Swift audio/calendar bridge, and React + Vite. No hosted backend, account system, analytics, Electron runtime, or bundled model weights.

This is the first development build. The interface and permanent library work; native audio capture, Calendar permissions and real model inference still need a supervised setup and reliability pass before relying on Patter for important meetings.

## Install and updates

Download the [latest Mac installer](https://github.com/taugr/patter/releases/latest). Drag Patter into Applications. Later, use Settings → Updates → Install and restart. Read [installation, release and recovery instructions](INSTALL.md), including the ad-hoc signing caveat. No Apple Developer membership or notarization is used.

## Run

Requires Node 24.14, pnpm (pinned in package.json), Rust 1.96, and Xcode Command Line Tools (Swift/C++). The checked-in Cargo/CMake configuration targets a portable M1 CPU baseline while keeping Metal acceleration enabled.

```sh
pnpm install
pnpm dev                 # browser design preview, with labelled examples
pnpm desktop             # native development app; stop a separate pnpm dev first
pnpm desktop:build       # standalone local Mac application
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
```

Application: `src-tauri/target/release/bundle/macos/Patter.app`. The local package is about 19 MB before models and saved recordings. It is not a signed/notarized distribution release. The native app starts with an empty library; the browser examples never populate it. One clearly labelled welcome/test note and synthetic audio were added during native QA on this Mac.

## Included

- Cream, ink and teal split-view notebook based on the selected third concept; Fraunces headings, Nunito Sans body text, generated Patter mark and Phosphor icons.
- Editable conversation titles/notes, summaries, decisions, action checkboxes, transcripts, search, Today filter and archive/restore.
- Immutable SQLite versions; restoring creates a new version. Existing audio attachments cannot be detached or repointed by ordinary saves. No delete/retention-cleanup commands.
- Audio import with a durable copy, playback, timestamp seeking, speed control, and consecutive chunk playback within the selected track.
- Swift ScreenCaptureKit bridge for microphone and computer audio; approximately five-second CAF chunks, unique capture-session filenames, startup recovery of surviving chunks, and WAV playback derivatives. No screen video is stored.
- Local Whisper inference via whisper.cpp/Metal; a user-selected GGML `.bin` file. Summaries via a local OpenAI-compatible HTTP server. Endpoints are restricted to loopback, with proxy forwarding and redirects disabled.
- EventKit calendar reading, including Google calendars already synced to macOS; upcoming events can create or reopen linked notes. No Google OAuth application is needed for this route.
- JSON conversation export and native library backup (SQLite online backup plus original recording files). Browser preview data uses IndexedDB and is separate from the permanent Mac library.

## First setup

**Calendar:** Add the Google account in macOS Internet Accounts and enable Calendars. Confirm the events appear in Apple Calendar, then use Patter Settings → Connect Mac calendar and grant access. macOS requires full calendar access for EventKit reads; Patter's implementation only reads. Events refresh on launch or when connecting again. Direct Google OAuth, calendar selection and background refresh are future work.

**Summaries:** Start a model in LM Studio's local server, then use `http://127.0.0.1:1234/v1`. For an existing Ollama server use `http://127.0.0.1:11434/v1`. Choose Find models, enter/select a model, and Save. The endpoint/model are preferences, not credentials. No cloud fallback is implemented.

**Transcription:** Obtain a compatible whisper.cpp GGML `.bin` model and select it in Settings. Model downloads are not automated. Import or record audio, open Transcript, and choose Transcribe recording. Language is auto-detected. Current capture transcription processes each chunk separately and labels the source (Microphone / Computer audio), not individual speakers. Longer track-level context and inference progress/cancellation need further work.

**Recording:** Record → Record microphone & computer audio. macOS may request microphone and screen/system-audio capture permissions. Capture was compiled but not activated during QA. Test permissions and playback on a short disposable conversation first.

## Permanent library

On this Mac: `~/Library/Application Support/gr.tau.patter/`.

- `patter.sqlite3`: current conversations, immutable versions and preferences.
- `recordings/<conversation-id>/`: original imported files and captured chunks; playback derivatives may also live here.
- `processing/`: retained conversion files from transcription attempts.

Patter never automatically purges these files. Archive changes visibility. Each committed edit/regeneration is versioned; draft keystrokes are coalesced with a 600 ms debounce, and normal closing/quit waits for pending saves. Forced termination can lose an unsaved draft or the currently open capture chunk. App retention cannot protect against disk failure or manual filesystem deletion: use library backups.

Backup restore is manual in this initial version. Quit Patter, preserve the current library separately, and restore the database and recordings into the same application-support location. Recording paths are absolute, so moving a backup to another account/path requires migration. Automated restore validation, crash fault injection, low-disk, long-call, sleep/wake and device-change tests remain outstanding.

## Verified here

- Production TypeScript/Vite build and Tauri `.app` packaging.
- Four frontend storage/search tests and ten Rust storage/model/update-safety tests.
- Browser flows: note editing, retained versions, restore, archive/restore, text search, action checkbox persistence, tabs, sample audio play/seek, desktop and compact layouts. No browser console errors in the final inspection.
- Native app launch, creating/editing a note, Cmd-Q and reopening with notes intact, importing synthetic WAV audio, and native playback.

Not yet verified: live capture, Google/EventKit permission flow, real Whisper or LLM output, full backup/restore round trip, long-session recovery/performance. No real microphone/system capture or personal calendar reading was performed.

## Attribution and licensing

Patter is a fresh implementation inspired by [Anarlog](https://github.com/fastrepl/anarlog); no upstream application source was copied. The bundled example conversation and narration are synthetic. The logo was generated for Patter. Fonts are Fraunces and Nunito Sans, with Phosphor icons.

No source-code license has been selected yet; public visibility does not grant a general license to this application's original code. Third-party components retain their own licenses; see `THIRD_PARTY_NOTICES.txt`.
