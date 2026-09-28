# Google Drive backups for Patter

Status: implemented locally; see [setup and validation](google-drive-backup.md). Live production OAuth, real Drive backup/restore, and LaunchAgent catch-up with Patter closed passed on 28 September 2026. The changed ad-hoc signature prevented unattended Keychain access after a test update. Explicit reconnection now creates a fresh protected credential item, preserves the backup identity and schedule, and has passed a real resumed background backup. Ad-hoc updates still require that reconnection; seamless migration is not claimed. Patter now has its own Drive folder and enabled schedule on the validation Mac. Existing Anarlog backup jobs were not changed.

## Intended experience

Settings → Backups:

- Connect Google Drive using the system browser.
- Create a dedicated Patter Backups folder. An existing-folder Picker is deferred.
- Enable nightly backup and choose a time; show the time zone explicitly.
- Show the account, destination, next run, last completed backup, progress, and any action needed.
- Offer Back up now, Open Drive folder, Restore from backup, Pause and Disconnect.
- Failures remain visible until resolved. Notify on actionable failure or overdue protection, not every routine success.

Separate source-device/library identifiers prevent two Macs from overwriting each other. This is backup and explicit restore; editing a Drive file does not change the local library.

The first implementation uses directly readable Drive files. Client-side encryption is deferred; retain the same snapshot and upload architecture if encryption is chosen. An encrypted option must use an established authenticated-encryption format/library and a separately saved recovery key, never a bespoke cipher.

## What is covered

- A consistent SQLite snapshot, including all conversations, archived items, transcript/summary history, decisions, action items and stored preferences.
- All retained original recordings and imported Anarlog session files/attachments.
- Readable per-conversation notes, generated summaries, timestamped transcripts and JSON exports when using the readable option.
- Summary-template instructions, selected model IDs/revisions, local model endpoint settings, calendar preferences and backup preferences.
- A manifest describing schema/app version, device/library identity, relative paths, file sizes, SHA-256 hashes and completion time.

OAuth refresh tokens and other credentials remain in macOS Keychain and are not exported to Drive. Reconnect Google on a restored Mac and grant that Mac's calendar/recording permissions again. A restored backup schedule remains paused until its destination/device identity has been reviewed.

Downloaded model weights and reproducible processing files are excluded by default to keep backups small; preserve their configuration and pinned model references. A manually selected external model file needs relinking or a separate explicit inclusion option. A backup must not claim these external dependencies are present when they are not.

## Architecture

Keep Tauri, Rust and SQLite. Add shared Rust backup code and a headless backup entry point packaged with the application. Call Google's HTTPS API directly using the existing HTTP stack. No hosted Patter backend, Python service, rclone installation or continuously running app window is required by this design.

1. **Authorization:** desktop OAuth, PKCE, state validation and a short-lived loopback callback in the system browser. Use offline access and store refresh credentials in Keychain. Request `drive.file`, covering files created by Patter and explicitly selected resources, rather than broad Drive access. Creating a fresh app-owned folder is the simplest initial destination.
2. **Capture:** flush saved UI edits when the GUI is present, then create a SQLite online backup. Derive exports and the referenced-file set from this frozen snapshot. Use a cross-process coordination mechanism shared by the GUI and worker; the current in-memory Activity guard is insufficient. Defer capture while recording/importing or while files are being finalized, and retry. Do not block normal work for the entire network upload.
3. **Stage:** create a stable local staging area with immutable files or verified copies/clones. Include original files referenced by every retained version and retained recording/import files needed for recovery. Treat an actively changing file as incomplete, not successfully backed up.
4. **Transfer:** hash files and upload only new or changed content. Reuse immutable audio across snapshots, avoiding a fresh full audio upload nightly. Use Drive resumable uploads for large recordings and persist upload checkpoints for retries. Respect rate limits and distinguish disconnected/offline/quota errors from successful backups.
5. **Commit:** upload a unique snapshot manifest only after every required object is present and verified. Incomplete runs stay incomplete and never replace the last successful backup. Keep prior manifests and content indefinitely; no automatic pruning or propagated deletion.
6. **Restore:** download into an isolated staging library, verify hashes and SQLite integrity, check schema compatibility, then remap all recording/import paths to the new Mac. Restore as a separate library or make a safety backup before explicit replacement. Never overlay live files while Patter is running.

Suggested Drive organization:

    Patter Backups/
      <Mac name>--<stable device ID>/
        <library ID>/
          snapshots/<UTC timestamp>--<run ID>/
            manifest.json
            patter.sqlite3.gz
            configuration.json
          conversations/<conversation ID>/
            revisions/<revision>/
              notes.md
              summary.md
              transcript.json
              conversation.json
          recordings/<conversation ID>/<hash>--<original filename>
          originals/<conversation ID>/<hash>--<original filename>

The manifest links exact Drive file IDs to paths and hashes. Readable exports are supplementary; the SQLite snapshot preserves complete history and is the authoritative restore input. Do not depend on Drive's own file revision retention for permanent history. Reuse unchanged exports/audio; compress each database snapshot. Storage will grow as new data/history is retained, so show usage and quota failures.

## Nightly scheduling

Patter installs and manages a per-user launchd LaunchAgent when nightly backup is enabled. Use calendar scheduling, with the configured time interpreted in that Mac's local time zone. Provide a single-run lock and persisted schedule state, so daylight-saving changes or repeated wake events cannot start overlapping runs.

A bundled headless worker can run with the Patter window closed. It runs in the signed-in user's session; it cannot back up while the Mac is powered off or the user is logged out. launchd calendar jobs catch up after sleep. Add explicit overdue checks at login/startup and bounded retry scheduling for missed power-off periods or unavailable networking. Coalesce missed nights into one fresh backup. If Keychain is locked, wait for access rather than showing success.

The worker must coexist safely with app updates, and its launch path must remain valid after replacement. Pause/coordinate active work during updates and verify the registered job afterward. Disabling backup unloads the schedule; disconnecting removes local credentials without deleting existing backups.

## One-time Google setup and first technical proof

Create or reuse an appropriate Google Cloud project, enable Drive API, and register a Desktop OAuth client. Register Picker configuration if supporting an existing-folder chooser. Use one stable OAuth client across Patter installations so backups remain accessible from another Mac after sign-in.

Do not leave the consent configuration in External/Testing for unattended use: Google's documented refresh-token lifetime for that mode is seven days with Drive scopes. Check the project's applicable basic verification/branding requirements; do not assume a public repository itself requires broad Drive scopes or a hosted service. Desktop client identifiers can be distributed; account refresh tokens must never enter source, logs or release artifacts.

First prove authorization, token refresh, narrow-scope folder access, and unattended Keychain access from the scheduled process. Specifically test again after an ad-hoc-signed Patter update. The existing unsigned/notarization-free distribution makes credential continuity a validation requirement; do not promise prompt-free nightly operation until that passes.

## Implementation sequence

1. Prove Google connection + Keychain + a headless scheduled worker, including an app upgrade. No real library uploads in this spike: use a small synthetic fixture.
2. Extract the current local backup code into a portable snapshot/export/restore module. Fix absolute-path restore limitations and add versioned manifests.
3. Add resumable incremental Drive uploads, completion verification and manual Back up now.
4. Add schedule controls, launchd registration, catch-up/retry behavior, status and failure messages.
5. Complete in-app restore selection and verify a restored library on another Mac/account path.
6. Release through Patter's existing updater. Leave the old Anarlog launchd backup untouched until the new backup and restore have been proven and retirement is explicitly requested.

## Acceptance checks

- First backup, unchanged repeat backup and one edited conversation: only expected new objects uploaded.
- Recover notes, summaries, configuration, historical versions and byte-identical recordings into an empty library under a different filesystem path.
- Recover imported Anarlog originals and archived conversations.
- Disconnect network mid-recording upload, restart the worker, then resume without a false complete snapshot or duplicate upload explosion.
- Handle Drive quota exhaustion, expired/revoked authorization, locked Keychain, missing source files and disk-full staging.
- Correct behavior with the GUI closed, Mac asleep at schedule time, power-off/login catch-up, an active recording and two Macs sharing the same parent folder.
- Test app replacement/updater coexistence and Keychain access after an ad-hoc-signed update.
- Confirm launchd registration and an actual scheduled remote completion; manual success alone does not prove scheduling.
- Confirm the source data and all previously completed backups remain intact.

## References

- [Google desktop OAuth, PKCE and loopback callbacks](https://developers.google.com/identity/protocols/oauth2/native-app)
- [Drive file scopes and long-lived authorization](https://developers.google.com/workspace/drive/api/guides/api-specific-auth)
- [OAuth refresh-token expiration, including Testing mode](https://developers.google.com/identity/protocols/oauth2)
- [Drive resumable uploads](https://developers.google.com/workspace/drive/api/guides/manage-uploads)
- [Google Picker for desktop apps](https://developers.google.com/workspace/drive/picker/guides/desktop-mobile-picker)
- [Apple calendar scheduling and sleep behavior](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/ScheduledJobs.html)
