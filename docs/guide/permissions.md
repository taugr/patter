# Permissions

Open the installed **Patter.app** from Applications to configure access. A browser preview or unbundled `tauri dev` build cannot set up the packaged app's permissions.

## Set up access on launch

From v0.6.6, Patter checks microphone and computer-audio access when its library opens. If access is missing, **Set up access** lists only the permissions that need attention. Calendar and notifications are included when you have enabled those features.

- **Allow** opens a new macOS permission request.
- **Open settings** takes you to the relevant macOS settings when access was denied.
- **Check again** refreshes permission status.
- **Later**, Close or Escape dismisses setup until the next launch. You can continue taking notes and importing audio.

Returning from macOS Settings refreshes the check while setup is open. If macOS asks you to quit and reopen Patter, do that before trying again. The check and setup buttons never start recording or create a conversation.

## What each permission does

| Access | Used for | Where to configure it |
| --- | --- | --- |
| Microphone | Recording your voice | System Settings → Privacy & Security → Microphone |
| Screen & System Audio Recording | Capturing audio playing on your Mac | System Settings → Privacy & Security → Screen & System Audio Recording |
| Calendars | Reading events from your Mac's connected calendar accounts | System Settings → Privacy & Security → Calendars |
| Notifications | Showing meeting reminders | System Settings → Notifications → Patter |

Patter saves computer audio, not screen video. Calendar access is read-only in Patter, although macOS requires **full access** to read events. Drive backup uses a separate Google sign-in; it does not grant Calendar access. Agent access is a separate opt-in setting under **Settings → Agents**.

## Set up recording from Settings

Open **Settings → General → Recording → Enable recording access**. Allow Microphone access, then Screen & System Audio Recording when prompted. This requests both permissions without recording.

If access was denied, expand **Permissions help** for shortcuts to the two macOS settings pages. Enable access there, then reopen Patter if requested. Screen access always has a settings shortcut in startup setup because its status check cannot distinguish a first request from a previous denial.

## Patter is missing from the list

macOS normally lists an app after it requests access; there is no manual add button for Calendar or Microphone access. Use **Enable recording access** or **Settings → Calendar → Connect calendar** from the packaged app first.

Use v0.6.2 or later for the Calendar entitlement fix, and v0.6.4 or later for the microphone entitlement fix. These fixes work with Patter's personal ad-hoc-signed build. If macOS or an administrator restricts access, Patter cannot grant it itself.

After an app update, macOS may require permission again. Try a short disposable recording and playback before relying on capture for an important meeting. Fresh startup consent prompts and reliable long-session capture still need supervised checks on physical Macs.
