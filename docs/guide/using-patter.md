# Use Patter

Settings has five tabs: **General**, **Calendar**, **Models**, **Library**, and **Agents**. Switching tabs keeps your edits. Use **Save changes** for model, template and reminder preferences; zoom and agent permissions apply immediately. Connection and backup messages stay beside the relevant controls, and setup help is expandable.

## Notes and history

Create a conversation, edit its title and notes, mark actions complete, and search your library. Saved versions remain available in history; restoring an earlier version creates a new version. Archive hides a conversation from the active list and can be undone.

Use **Export conversation** for a JSON copy of a conversation and its saved history. Use a [library backup](library.md#make-an-independent-backup) to include the original audio.

## Recordings and transcription

On launch, the packaged app offers **Set up access** when recording permissions are missing. You can grant access, open macOS settings or choose Later. The check never starts recording. See [permissions and recovery](permissions.md).

The sidebar's **Record → Record microphone & computer audio** creates a new conversation. Use **Stop** when finished; Patter saves the recording. It retains audio chunks and imported originals in its local library; computer-audio capture does not save screen video.

**Import audio** adds a file to the currently selected conversation. Choose and download a model under **Settings → Models → Transcription**, then use **Transcript → Transcribe recording**. [Local models and templates](models.md) covers Parakeet v3, Whisper, download verification and repair.

Test a short disposable recording and playback before an important meeting. Live capture, long sessions and accuracy across devices still need supervised checks.

## Summaries and calendar

Connect a local OpenAI-compatible model server under **Settings → Models → Summaries**, then use **Summary → Create summary**. Choose default template guidance in Settings or expand **Template** in a conversation to override it. [Summary setup](models.md#connect-a-summary-model) explains local server addresses, templates and preserved generated versions.

Connect calendars already present in Apple Calendar, including a Google account added to macOS Internet Accounts. Upcoming events can open notes, and Google Meet or Zoom events can offer **Join**. Configure reminder timing, sound, title visibility and calendars under **Settings → Calendar**. Patter must remain open to send reminders. [Calendar and meetings](calendar.md) explains linking, joining and recording from reminders.

## Zoom and keyboard shortcuts

Choose **Settings → General → Zoom**, use the **View** menu, or press:

| Shortcut | Action |
| --- | --- |
| ⌘+ (or ⌘=) | Zoom in |
| ⌘− | Zoom out |
| ⌘0 | Reset to 100% |

Patter supports 75% to 200% and remembers zoom on each Mac. It applies immediately and is separate from library backup preferences. When a Settings tab has keyboard focus, use Left/Right arrows to switch tabs, or Home/End to select the first or last tab.

## Move from Anarlog

Copy your Anarlog 1.0.27 Content folder and use **Settings → Library → Import from Anarlog** to preview and import it. Patter preserves the source and skips unchanged repeat imports. Follow [storage and Anarlog import](library.md) for transfer steps, retained files, backup formats and migration limits.

## Connect an agent

Patter's local MCP server lets Codex, Claude or another stdio client search and work with your library while Patter is open. Access starts off, with separate read, edit and processing switches. The [agent connection guide](../agent-access.md) includes copyable setup prompts and commands; check the connection with `patter_status` before reading conversations.
