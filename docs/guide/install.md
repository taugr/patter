# Install and update

Patter is a personal preview for **Apple Silicon (M1 or newer) and macOS 15 or later**.

## Install

1. Download the DMG from the [latest release](https://github.com/taugr/patter/releases/latest).
2. Open it and drag Patter into **Applications**. Eject the DMG.
3. Open Patter from Applications. The current build is ad-hoc signed and is not notarized. If macOS blocks it, use the app-specific **Open Anyway** control in **System Settings → Privacy & Security**. Keep Gatekeeper enabled.
4. [Create your first note](getting-started.md) and configure models only if you want transcription or summaries.

Patter's library is separate on each Mac. Replacing the app keeps the library at `~/Library/Application Support/gr.tau.patter/`; an update does not synchronize meetings.

## Update

Patter checks for updates when the library opens. You can also choose **Patter → Check for Updates**. When an update is available, use **Settings → General → Updates → Install and restart**. It waits if recording, processing, importing, saving, or backing up is active.

The updater verifies the signed release archive and makes a database snapshot before replacement. Original recordings stay in the library. For a separate copy that includes audio, use **Settings → Library → Back up library** or [Google Drive backup](../google-drive-backup.md).

The ad-hoc signature may cause macOS to ask for app approval or recording/calendar permissions again. If Drive backup needs attention after an update, reconnect it with your original Google Desktop OAuth JSON. [Drive backup and restore instructions](../google-drive-backup.md#scheduling-and-status) explain that recovery path.

For release details and manual recovery, read the [repository's installation reference](https://github.com/taugr/patter/blob/main/INSTALL.md).
