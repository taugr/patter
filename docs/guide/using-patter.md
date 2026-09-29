# Use Patter

Settings has five tabs: **General**, **Calendar**, **Models**, **Library**, and **Agents**. Switching tabs keeps your edits. Use **Save changes** for model, template and reminder preferences; zoom and agent permissions apply immediately. Setup instructions are behind expandable help links.

## Notes and history

Create a conversation, edit its title and notes, mark actions complete, and search your library. Saved versions remain available in history; restoring an earlier version creates a new version. Archive hides a conversation from the active list and can be undone.

## Recordings and transcription

On launch, the packaged app checks microphone and computer-audio access and offers **Set up access** if anything is missing. Calendar and notification access are included only when those features are enabled. Choose **Allow** for a new request or **Open settings** if access was denied. Returning from macOS Settings refreshes the check; macOS may require you to reopen Patter. **Later** dismisses setup until the next launch. Checking access never starts a recording or creates a conversation.

Import an audio file into a conversation, or choose **Record → Record microphone & computer audio**. macOS can ask for microphone and screen/system-audio capture access. Patter stores audio chunks and imported originals in its local library. Computer audio and microphone labels identify tracks; they are not speaker diarization.

To set up permissions before recording, open **Settings → General → Recording → Enable recording access** in the packaged app. This requests access without creating a conversation or capturing audio. macOS lists Patter after it requests access. Allow Microphone access, then Screen & System Audio Recording when prompted. If access was denied, use the shortcuts under **Settings → General → Recording → Permissions help**, allow Patter, then quit and reopen it. No screen video is saved. Release checks verify the audio entitlement on both the app and recording helper, plus their consent descriptions.

In **Settings → Models → Transcription**, choose Parakeet v3, Whisper Base, or Whisper Small, select **Download**, then **Save changes**. Downloads come from Hugging Face and are verified before use. Once installed, transcription runs offline. You can also select an existing compatible whisper.cpp GGML `.bin` file. Open a conversation's **Transcript** tab and choose **Transcribe recording**.

Transcription on a short synthetic sample has been verified. Long calls, live capture, and model quality across accents and devices need further checks. Try a short disposable recording before an important meeting.

## Summaries and calendar

Summaries require a local OpenAI-compatible model server. In **Settings → Models → Summaries**, use `http://127.0.0.1:1234/v1` for LM Studio's local server or `http://127.0.0.1:11434/v1` for an existing Ollama server, find a model, and save it. Patter has no cloud fallback. In **Settings → Models → Templates**, choose a default or edit instructions; a conversation can override that choice. Past generated versions retain their template and model details.

To connect events, add your account in macOS **Internet Accounts**, enable Calendars, and check that events appear in Apple Calendar. Then use **Settings → Calendar → Connect calendar** and grant full Calendar access. Patter reads events and can create or reopen a linked note. It does not edit calendar events.

Use v0.6.2 or later for the Calendar entitlement fix. Settings shows permission status and keeps connection errors beside the button. If Patter is missing from macOS Calendar permissions, click **Connect calendar** in the packaged app first; macOS lists apps after they request access. If access is denied, use **Calendar permissions** to enable Patter, then reconnect. **Check again** refreshes the status, and **Connection help** includes permission shortcuts and Google account setup. The same dialog links to Notification, Microphone and Screen & System Audio settings. Calendar consent has been confirmed in a packaged local test build.

**Join meeting** opens a Google Meet or Zoom link in your default browser, which can hand Zoom links to the Zoom app. Look for **Join** under Upcoming, or **Join meeting** in linked notes and reminders. Patter checks the event’s URL, location and invitation notes. Zoom passcodes in the link are retained. Joining does not start recording. Events without a recognised link have no Join button; new linked notes keep the link for later use.

Meeting reminders are optional. After connecting Calendar, enable them in **Settings → Calendar**, choose the lead time and calendars, then save. Patter must remain open for reminders to run. A reminder can open notes or offer a recording confirmation; it never starts recording automatically. Use **Test notification** to check macOS delivery.

## Move from Anarlog

If you have an Anarlog 1.0.27 library, make a copy of its Content folder, then use **Settings → Library → Import from Anarlog** to preview and import it. Patter keeps source files and skips unchanged repeat imports. Read the [format and transfer limits](https://github.com/taugr/patter/blob/main/INSTALL.md#import-an-anarlog-1027-library) before moving a large library.

## Connect an agent

Patter's local MCP server can let Codex, Claude, or another stdio client search and work with your library while Patter is open. Access is off until you enable it, and read, edit, and processing permissions are separate. Follow the [agent connection guide](../agent-access.md) and check the connection with `patter_status` before reading conversations.
