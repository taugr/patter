# Google Drive backup

Implemented in the local development build; not yet published. Patter uses its existing Rust/Tauri process and Google's API directly. No hosted service or separate backup-tool installation is needed.

## Connect once

1. Keep Patter in **Applications**, so its background launch path remains stable.
2. In [Google Cloud Console](https://console.cloud.google.com/), create or choose a project and enable **Google Drive API**.
3. Configure Google Auth Platform branding/audience. Create a **Desktop app** OAuth client and download its JSON setup file. Keep this file outside the public repository. Use the same client on your other Macs, including when restoring.
4. For ongoing use, move an External consent configuration out of **Testing**: Drive refresh tokens otherwise expire after seven days. Follow the console's applicable consent/branding requirements. See [Google's token-expiration documentation](https://developers.google.com/identity/protocols/oauth2#expiration).
5. In **Settings → Google Drive backup → Connect Google Drive**, select that JSON file, then sign in using your browser. Allow Patter's `drive.file` access. Patter creates/reuses its own private **Patter Backups** folder; it does not request access to all your Drive files.
6. Choose **Back up now** for the first backup. Then choose your local nightly time and **Enable nightly backups**.

The JSON file configures the desktop OAuth client. Account refresh credentials are stored in macOS Keychain, never in the library, backups, logs, source, or release workflow. The OAuth connection is separate from the Mac Calendar connection. Connecting does not enable nightly uploads until you choose Enable.

## What gets backed up

- A compressed, consistent SQLite snapshot containing all current/archived conversations, immutable previous versions, transcripts, summaries, action items and preferences.
- Every file in the recordings directory, including imported Anarlog originals and attachments.
- Readable notes, summary Markdown, transcript JSON and conversation JSON for current revisions.
- Settings, edited summary-template instructions, selected model references, the pinned model catalogue, and backup time/preferences.
- A manifest mapping relative paths to immutable Drive file IDs, sizes and SHA-256 hashes.

Downloaded model weights, caches, temporary processing files, credentials and OS permissions are excluded. Models can be downloaded again; a manually selected external Whisper file needs relinking. The complete database preserves earlier text versions even if they predate readable exports.

Files are **readable in Drive**, without additional client-side encryption. Each device/library has a separate folder. Patter reuses unchanged remote files, creates new objects for changed content, and never prunes old snapshots or propagates local deletion. Google storage usage therefore grows with your retained history. Editing Drive files does not edit the local library.

A backup is complete only after all files and its final manifest have been verified on Drive. Interrupted uploads retain local checkpoints and resume later. Verified temporary upload copies are cleaned up after success; originals, historical versions and remote backups remain untouched. Capturing a snapshot defers while recording or other library operations are active. A frozen upload can coexist with ordinary editing; app updates wait until the worker finishes.

## Scheduling and status

Enabling installs `~/Library/LaunchAgents/gr.tau.patter.backup.plist`. It runs the installed executable with `--backup-worker` in your signed-in user's session. The worker never opens a window and exits when finished or nothing is due. Calendar scheduling plus a 15-minute check provides retries and catch-up after sleep/login. Missed nights coalesce into a current backup; a resumed old snapshot is completed before the next fresh one.

The Mac must be awake, online and signed in, with accessible login Keychain. It cannot upload while powered off or logged out. Patter does not wake a powered-off Mac. Time follows the Mac's local time zone. A changed app location requires saving the schedule again.

Settings shows the last verified remote completion and errors. The sidebar flags failed or overdue backups while Patter is open. There are no macOS push notifications in this version. Locked Keychain or changed ad-hoc signatures cause a retryable status instead of a hidden authentication prompt. **After an ad-hoc app update, reconnect Drive when Patter shows “Backup needs attention.”** Choose **Reconnect Google Drive**, select the same Google Desktop OAuth JSON and sign in to the same account. Patter creates a fresh Keychain item for the updated app and keeps your library identity, Drive folder, backup history and schedule. A due or pending backup resumes automatically. This is a one-time reconnection for each changed ad-hoc build, not seamless credential migration. Keep the Google setup JSON available on each Mac. The old protected Keychain item is left untouched; no ACL is widened and no token is written to the library or backup.

Pause unloads Patter's schedule. Disconnect also removes this library's local Keychain credential. Both keep existing Drive backups. Existing Anarlog launchd jobs are never changed.

## Restore on this or another Mac

1. Install Patter and connect the same Google account using the same OAuth client setup.
2. Choose **Restore a backup**, select a completed snapshot, then **Download and verify restore**.
3. Patter checks all hashes, SQLite integrity and schema compatibility, reconstructs immutable history, and remaps recording/import paths for this Mac.
4. Choose **Restart and open restored library**. Your previous complete library stays beside the active library in `~/Library/Application Support/Patter-before-restore-<id>/`.
5. Reconnect Drive, review/re-enable scheduling, download models and grant this Mac's Calendar/recording permissions again.

Restore replaces the active library after restart; it does not merge libraries. Restoring never overlays an open database. A journal handles interruption between directory renames. Restored schedules are paused and receive new device/library IDs. Original safety folders remain indefinitely.

## Validation completed and still needed

Automated tests cover portable restore with byte-identical audio, archived items, previous versions, imported originals, preferences, immutable triggers, corrupt/missing files, path traversal/symlinks, unchanged content hashes, cross-process lock behavior, catch-up/retry scheduling, OAuth callback state, interrupted/resumed uploads against a local HTTP fixture, duplicate-upload avoidance, and rejecting remote checksum mismatches.

Live validation on 28 September 2026 confirmed production OAuth connection, a real Drive upload, LaunchAgent catch-up completion with Patter closed, and restore after restart. All 22 backup files were verified remotely; all three conversations, nine saved versions and preferences matched the preflight database exactly after restore, and all eight recording files had identical SHA-256 hashes. The previous library remained in its safety folder. The LaunchAgent exited successfully without a foreground app.

The initial update test reproduced a Keychain failure when the ad-hoc code signature changed. The recovery fix was then validated with a local 0.3.1 candidate: Patter detected the inaccessible legacy credential on launch, displayed a reconnect action, renewed the same Google account through the original setup JSON, and saved credentials in a fresh protected Keychain item. Library/device IDs, account/client, Drive folder and enabled scheduling were preserved. The due LaunchAgent backup resumed after reconnection with Patter closed and verified all 22 files. No Keychain password prompt or broader access permission was needed. This validates explicit reconnection, not seamless credential migration. The candidate was not published.

Exact wall-clock nightly triggering, offline/quota/revoked-authorization behavior, sleep/login and a second physical Mac remain live checks. Keep the existing Anarlog backup in place while these remaining cases are resolved.

See the [design and acceptance plan](google-drive-backup-plan.md).
