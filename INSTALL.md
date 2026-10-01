# Install and update Patter

Patter is a personal preview for **Apple Silicon (M1 or newer), macOS 15+**.

## Install once

1. Download the DMG from [the latest release](https://github.com/taugr/patter/releases/latest).
2. Open it and drag Patter into Applications. Eject the DMG.
3. Open Patter from Applications. This build uses free ad-hoc signing, without an Apple Developer certificate or notarization. If macOS blocks it, use the app-specific **Open Anyway** option in System Settings → Privacy & Security. Do not disable Gatekeeper globally.
4. Configure local models and Calendar in Settings as described in the README.

An existing 0.1.0 development build has no updater: replace it manually once. App replacement keeps `~/Library/Application Support/gr.tau.patter/` intact. Each Mac has its own library; updates do not synchronize meetings.

## Update

Patter checks quietly when the library opens. An available update appears in Settings → General → Updates; the Patter menu also has **Check for Updates**. Choose **Install and restart** when ready. Nothing installs or restarts without that click.

Updates are downloaded over HTTPS, verified against Patter's embedded public key, and checked against their signed release version. The native activity lock refuses updates while recording, processing, importing, saving, or backing up. Pending frontend edits are flushed first. Failed downloads, verification or backups leave the app installed and show a retryable message.

A consistent database snapshot is retained in the library's `backups/` directory before replacement and schema migrations. It contains conversations, transcripts, versions and settings; original audio stays in `recordings/` and is never modified by the updater. For an independent copy including audio, use Settings → Library → Back up library.

Ad-hoc signing changes the app/helper code identity between builds, so macOS can request app approval or microphone/system-audio/calendar permissions again after an update. v0.6.8 removes screen capture and its misleading permission gate, but does not guarantee grant continuity. Stable Developer ID signing requires an Apple signing identity that is not configured in this repository. No permissions are reset or transferred by the updater. In Settings → General → Recording, **Check again** refreshes microphone status. System audio is checked by macOS when recording starts; use **System Audio Recording Only** within the Screen & System Audio Recording page, and reopen Patter only if macOS asks. Live audio capture and real model inference still need supervised reliability testing.

## Publish a new version

Commit and push normal changes to `main`; GitHub runs checks without publishing an app update. When you want to ship:

```sh
pnpm release:patch       # or pnpm release:minor
```

This command requires a clean, fully pushed `main`, increments all version files, creates a release commit and tag, and **pushes both**. A version tag runs tests, builds the ad-hoc DMG and signed updater archive, validates signatures/checksums, uploads a complete draft, then publishes it. Follow [Actions](https://github.com/taugr/patter/actions). An ordinary source push alone does not update installed apps.

`PATTER_AUTO_PUBLISH=true` enables publication after all gates pass. Set it to false to hold future builds as drafts. Published artifacts are immutable; a fix needs a higher version. Concurrent release jobs serialize, and older versions cannot supersede a newer stable release. During initial upgrade proof, `PATTER_TEST_FEED` may override the feed in a bootstrap test build; clear it before stable publication.

## Signing key and recovery

The updater's free signing key is separate from Apple code signing. GitHub Actions secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` are available only to the release build step. Never put either in source, logs or an app bundle. An encrypted local copy is kept in the ignored `.secrets/` directory; preserve it and its password separately in your own secure backup. GitHub secrets cannot be downloaded later.

If the key is lost, existing clients cannot trust a new signing identity automatically: distribute a manual replacement or use a deliberate key migration while the old key still exists. No automatic database rollback is promised. If a release fails after a schema migration, prefer a corrected newer release. For manual recovery, quit Patter, copy the entire current library somewhere safe, and restore a compatible database snapshot together with its original recordings. Do not overwrite or delete the only copy. Backups currently contain absolute recording paths; moving between accounts requires path migration.

## Validation limits

Automated gates cover frontend persistence/search, native activity exclusion, immutable versions/audio references, schema rejection, failed migration rollback and database backup restoration. Signature/packaging checks validate release assets before publication. Real capture, Calendar access, interrupted downloads, actual disk exhaustion, and permissions across two physical Macs remain separate verification tasks.

## Import an Anarlog 1.0.27 library

Available from Patter v0.4.0.

1. On the old Mac, open Anarlog's **Settings → Storage** and locate **Content**. Its default is `~/Library/Application Support/hyprnote/`; a custom location may differ.
2. Quit Anarlog so its files stop changing. Copy the entire Content folder (including `sessions/` and its nested folders) to the new Mac using AirDrop or an external drive. Keep the original.
3. In Patter, open **Settings → Library → Import from Anarlog → Choose Anarlog folder** and select the copied Content folder, or its `sessions/` folder.
4. Review the conversation list and import notes, then choose **Import conversations**. Large libraries take longer because every file is checked and copied.

The importer targets the [Anarlog desktop_v1.0.27 session format](https://github.com/fastrepl/anarlog/tree/desktop_v1.0.27/apps/desktop/src/store/tinybase/persister/session): `_meta.json`, `_memo.md`, summary Markdown files, `transcript.json`, and retained `audio.mp3`, `audio.wav`, or `audio.ogg`. It includes nested meeting folders. It does not support a newer SQLite-only library or a Markdown-only export.

Titles, creation dates, note/summary text and transcript timings are imported. Explicit word speaker labels are retained; otherwise the original microphone/computer channel is used. Original JSON, Markdown, attachments and other files in each session folder are copied unchanged to `recordings/<conversation-id>/anarlog-source/`, so they are also included in Patter's full library backup. Rich formatting, participant mappings, tags and template metadata remain available in those original files; custom templates, contacts, chat history and application settings are not converted into Patter features. No recordings that Anarlog already deleted can be recovered.

Import never writes to the source. Patter snapshots its database first, verifies copied files and commits the batch together. Unchanged repeat imports are skipped even after moving the copied folder. Changed source sessions become separate conversations, preserving edits made in Patter. Invalid files or links stop the import with an error. If copying fails, any copied files remain in Patter's recordings folder, but no partial batch appears in the library; retry is safe. Nothing is uploaded.

## Google Drive backups

Patter v0.4.0 includes **Settings → Library → Google Drive** for manual/nightly backups and verified restore on another Mac. It requires one-time Google Desktop OAuth setup and your sign-in. Follow the [setup guide](docs/google-drive-backup.md). This creates Patter's own launchd job only when you enable nightly backups; existing Anarlog backup jobs are untouched. Real Drive backup/restore and LaunchAgent catch-up with the app closed have passed. After an ad-hoc app update, use **Reconnect Google Drive** if prompted, select the original Google setup JSON and sign in again. The verified recovery path preserves your backup history and schedule and resumes due backups. Reconnection is required because each changed ad-hoc signature has a different Keychain identity.
