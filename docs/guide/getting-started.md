# Get started

Patter keeps its library on your Mac. You can begin with a note, then add a recording or connect a local model when you need one.

## 1. Install

[Download the latest release](https://github.com/taugr/patter/releases/latest), drag Patter into Applications, and open it. The current preview supports Apple Silicon and macOS 15 or later. See the [installation and update guide](install.md) for the macOS approval step and update behavior.

## 2. Make a note

Create a conversation and write in Notes. Patter saves committed edits as versions, so you can revisit earlier text. Search finds conversations, and Archive removes one from the active list without deleting it.

## 3. Add audio and a transcript

For a first check, import a short recording. To capture a new one, choose **Record → Record microphone & computer audio** and grant the macOS permissions requested. Patter's live capture still needs a supervised reliability pass; test with disposable audio first.

Open **Settings → Models → Transcription**, choose and download a model, then open the conversation's **Transcript** tab and choose **Transcribe recording**. Downloads are verified; inference runs on your Mac. [Learn about recordings and models](using-patter.md#recordings-and-transcription).

## 4. Add a summary or calendar

To generate summaries, start a local OpenAI-compatible model server such as LM Studio or Ollama and set its loopback URL in Patter. Choose a summary template in Settings or for a conversation. To see upcoming meetings, connect a calendar already present in macOS Calendar. [See the setup details](using-patter.md#summaries-and-calendar).

## 5. Protect the library

Patter retains recordings and saved versions, but that alone does not protect against disk failure. Use **Settings → Library → Back up library** for an independent local copy, or set up [optional Google Drive backup](../google-drive-backup.md). A Drive backup includes original recordings and the full history, but excludes downloadable models and credentials.
