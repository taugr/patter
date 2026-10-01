# Permissions

Open the packaged **Patter.app** to configure access. A browser preview or unbundled `tauri dev` build cannot configure the packaged app's grants.

## Set up access on launch

From v0.6.8, Patter checks microphone access when its library opens. **Set up access** lists permissions that need attention; Calendar and notifications are included when you enable those features. **Allow** requests only the chosen permission, **Open settings** handles a previous denial, and **Check again** reads fresh status. Restricted access cannot be granted by Patter. Later, Close or Escape dismisses setup until the next launch.

System audio is checked by macOS when recording starts. There is no public non-prompting Core Audio permission query on the supported macOS versions, so Patter does not label it denied or allowed based on the separate screen grant. Setup and refresh never start a recording, create an audio tap or request screen capture.

## What each permission does

| Access | Used for | Where to configure it |
| --- | --- | --- |
| Microphone | Recording your voice | System Settings → Privacy & Security → Microphone |
| System Audio Recording Only | Capturing audio playing on your Mac | System Settings → Privacy & Security → Screen & System Audio Recording → System Audio Recording Only |
| Calendars | Reading events from connected calendar accounts | System Settings → Privacy & Security → Calendars |
| Notifications | Meeting reminders | System Settings → Notifications → Patter |

Patter uses Core Audio taps for system audio and AVAudioEngine for the microphone. It does not enumerate screens or windows, capture images, or require screen-recording access. Calendar access is read-only in Patter, although macOS requires full access to read events. Drive backup uses separate Google sign-in. Agent access remains a separate opt-in setting.

## Refresh and recover

Open **Settings → General → Recording**. Microphone status refreshes when you return to Patter, when the window becomes visible, and with **Check again**. **Allow microphone** requests only microphone consent. If denied, **Open microphone settings** opens macOS settings instead of repeating a request that macOS cannot grant. A failed status check stays visible and can be retried.

**Permissions help** opens the macOS microphone or system-audio settings. An enabled System Audio Recording Only grant does not need a screen grant. macOS validates audio consent when you explicitly start recording; complete its prompt if shown. Reopen Patter only if macOS asks. A failed start leaves an error beside the recording confirmation and preserves the selected conversation for retry.

macOS lists an app after a consent request; there is no manual add button for Microphone or Calendar. System audio may appear after the first recording start, rather than a Settings refresh. Patter never starts recording to populate this list.

## Updates and signing

Current releases use ad-hoc signing. The app/helper code identity changes between builds, which can make macOS request permission again after an upgrade even though the bundle identifier is unchanged. v0.6.8 fixes the audio-only API and false screen-permission denial; it does not establish stable Apple signing identity or guarantee permission continuity. Stable Developer ID signing requires an Apple account/certificate not currently configured. No TCC reset, permission grant transfer or security bypass is performed.

Long-session capture, actual macOS consent, device changes, and permission continuity still require supervised testing on physical Macs. Automated capture checks use generated audio and mock resources; no live recording was made for this fix.
