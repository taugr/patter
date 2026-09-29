# Library, backups and Anarlog import

Each Mac has its own Patter library at:

```text
~/Library/Application Support/gr.tau.patter/
```

App updates keep this folder. Installing Patter on another Mac does not synchronise conversations.

## What Patter keeps

| Location | Contents |
| --- | --- |
| `patter.sqlite3` | Conversations, transcripts, summaries, action items, preferences and saved versions |
| `recordings/` | Recorded audio chunks, imported audio and original Anarlog session files |
| `models/` | Downloaded transcription models |
| `backups/` | Database safety snapshots made before updates and other protected operations |

Patter uses SQLite for the live library. Google Drive snapshots also include readable notes and summary Markdown, plus transcript and conversation JSON. The database preserves earlier versions as well as the current text. Editing a backup export does not edit Patter's library.

Archive hides a conversation from the active list and can be undone. Restoring a saved version creates a new version. Recordings and conversation history are not automatically purged.

## Make an independent backup

In the Mac app, choose **Settings → Library → Local backup → Back up library**, then select a destination folder outside the Patter library. This creates a consistent copy of `patter.sqlite3` and `recordings/`, with a README explaining manual recovery. An update's automatic database snapshot is kept inside the library and does not provide the same protection as an independent copy of the audio.

The local backup keeps absolute audio paths. Quit Patter and preserve the current library before any manual recovery. Moving that backup to a different account requires path migration; use Google Drive restore for a supported portable restore.

For nightly uploads and restore on another Mac, follow the [Google Drive backup guide](../google-drive-backup.md). It explains the separate Google OAuth setup, configurable time, catch-up after sleep, reconnection after app updates and verified restore. Restore replaces the active library rather than merging it, and preserves the previous complete library in a safety folder.

The conversation's **Export conversation** button exports its data and saved history as JSON. It is not a full audio backup. Browser-preview exports also exclude audio.

## Move an Anarlog 1.0.27 library

1. On the old Mac, open Anarlog's **Settings → Storage** and locate **Content**. The default is `~/Library/Application Support/hyprnote/`; a custom location may differ.
2. Quit Anarlog and copy its entire Content folder, including `sessions/` and nested folders, to the new Mac using AirDrop or an external drive. Keep the original.
3. In Patter, open **Settings → Library → Import from Anarlog → Choose Anarlog folder**. Select the copied Content folder or its `sessions/` folder.
4. Review the preview and choose **Import conversations**.

The importer supports Anarlog 1.0.27 session folders containing `_meta.json`, `_memo.md`, summary Markdown, `transcript.json` and retained audio. A newer SQLite-only library or a Markdown-only export is not supported.

Patter imports titles, dates, notes, summaries, transcript timing and retained audio. Original JSON, Markdown, attachments and other session files are copied unchanged into each conversation's `recordings/<conversation-id>/anarlog-source/` folder and are included in full backups. Custom templates, contacts, chat history, tags and application settings are not converted into Patter features; original metadata remains with the copied source files. Audio previously deleted by Anarlog cannot be recovered.

Import never changes the source or uploads anything. It snapshots Patter's database, verifies the copied files and commits the batch together. Unchanged repeat imports are skipped; changed source sessions become separate conversations so existing Patter edits are preserved. Invalid files or links stop the import with an error. If a copy fails, copied files may remain in recordings, but no partial batch appears in the conversation list; retry is safe.

Keep the old Anarlog library and backup until you have checked the imported conversations, transcripts and audio in Patter. Existing Anarlog backup jobs are not changed by Patter.
