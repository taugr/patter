# Get started

Patter keeps its library on your Mac. You can begin with a note, then add a recording or connect a local model when you need one.

## 1. Install

[Download the latest release](https://github.com/taugr/patter/releases/latest), drag Patter into Applications, and open it. The current preview supports Apple Silicon and macOS 15 or later. See the [installation and update guide](install.md) for the macOS approval step and update behavior.

If **Set up access** appears, allow microphone and computer-audio access or choose **Later** to start with notes. The setup check never starts recording. [Permission setup and recovery](permissions.md) explains the macOS settings shortcuts.

## 2. Make a note

Create a conversation and write in Notes. Patter saves committed edits as versions, so you can revisit earlier text. Search finds conversations, and Archive removes one from the active list without deleting it.

## 3. Add audio and a transcript

For a first check, import a short recording. To capture a new one, choose **Record → Record microphone & computer audio** and grant the macOS permissions requested. Patter's live capture still needs a supervised reliability pass; test with disposable audio first.

Open **Settings → Models → Transcription**, choose and download a model, then open the conversation's **Transcript** tab and choose **Transcribe recording**. Downloads are verified; inference runs on your Mac. [Choose a model and repair downloads](models.md#download-a-transcription-model).

## 4. Add a summary or calendar

To generate summaries, [connect a local model server and choose a template](models.md#connect-a-summary-model). To see upcoming meetings, [connect a calendar already present in macOS Calendar](calendar.md). You can then configure reminders or join Google Meet and Zoom calls from Patter.

## 5. Protect the library

Patter retains recordings and saved versions, but that alone does not protect against disk failure. Use **Settings → Library → Back up library** for an independent local copy, or set up [optional Google Drive backup](../google-drive-backup.md). A Drive backup includes original recordings and the full history, but excludes downloadable models and credentials.

Moving from Anarlog? Follow the [1.0.27 import guide](library.md#move-an-anarlog-1-0-27-library) before copying your old library.
