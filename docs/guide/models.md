# Local models

Transcription and summaries use separate models. Download a transcription model in Patter; run a local model server for summaries.

## Download a transcription model

1. Open **Settings → Models → Transcription**.
2. Select a model and choose **Download**. Wait for download and verification to finish.
3. Choose **Save changes**.
4. Open a conversation with saved audio, switch to **Transcript**, then choose **Transcribe recording**.

The current bundled catalogue includes:

| Model | Approximate download | Runtime |
| --- | --- | --- |
| Parakeet v3 | 632 MB | Core ML on Apple Silicon |
| Whisper Base | 148 MB | whisper.cpp |
| Whisper Small | 488 MB | whisper.cpp |

Sizes describe model files before filesystem overhead. Downloads come from pinned Hugging Face revisions and are checked against expected file sizes and SHA-256 hashes. Once installed, transcription runs locally without a network connection.

**Verify / repair** checks an installed model and downloads files that need repair. **Cancel download** stops the current download; selecting Download again can resume partial files where the server supports it. Model weights are excluded from library backups and can be downloaded again on another Mac.

For an existing whisper.cpp GGML `.bin` model, select **Whisper — choose a file**, choose the file and save. Keep that file available; an external model needs relinking after moving to another Mac. Patter does not accept arbitrary Parakeet model folders. New compatible model versions are added through app updates rather than silently replacing your selected model.

Transcripts retain timing and audio-track information. Microphone and computer-audio labels identify tracks; they do not identify individual speakers. A short synthetic transcription has been verified. Check real calls, accents and longer recordings before relying on the result.

## Connect a summary model

Start a local OpenAI-compatible server with a model loaded, then open **Settings → Models → Summaries**.

| Server | Local server address |
| --- | --- |
| LM Studio | `http://127.0.0.1:1234/v1` |
| Ollama | `http://127.0.0.1:11434/v1` |

Enter the address under **Local server**, choose **Find models**, select or enter a model name, then **Save changes**. Use the port your server actually runs on and include `/v1`. Patter accepts loopback addresses such as `localhost`, `127.0.0.1` and `::1`; remote model servers are rejected and there is no cloud fallback.

Open a conversation's **Overview** tab and choose **Create summary**. If no models are found, check that the server is running and has a model loaded. If generation fails, check the saved address, model name and the server's status.

## Summary and action items

Every new summary uses the same format:

- **Summary** has meaningful subsections for the items actually discussed. Decisions, alternatives and unresolved questions appear within the relevant topic.
- **Action items** lists concrete actions supported by the conversation. Owners and deadlines appear only when stated; an empty list displays **No action items were agreed.** Suggestions and observations must not become invented commitments.

There are no template selectors or custom-instruction editors. Old global templates, custom instructions and per-conversation overrides remain in saved settings and records for compatibility, but no longer influence generation. Opening an existing conversation does not convert or regenerate its summary. Legacy decisions and completed actions remain readable.

Regenerating replaces generated content in the current version and keeps the previous summary, decisions, actions and their completion state in history. New versions record the canonical instructions, format version, model and original source revision. Notes, transcript and audio remain intact. No database migration is needed. JSON exports and Drive backups retain both old and new versions; new topic boundaries are portable `###` headings in the saved summary string. Older Patter builds can read that string as plain text.

Review generated summaries and actions for accuracy. The prompt requires faithful output, but schema validation cannot independently verify a model's factual claims.
