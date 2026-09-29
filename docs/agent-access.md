# Connect a local agent to Patter

Available from v0.5.0. Keep the Mac app running while using the connection.

## Enable access

1. Open **Settings → Agents** and enable **Read entire library**.
2. Optionally enable **Edit notes, titles and actions** and **Transcribe and summarize**. These switches save immediately. Reading alone cannot change library content.
3. Expand **Connection setup** and choose **Copy settings**. Use this generated configuration to connect your client below.

## Set up with a prompt

After enabling access, paste this prompt into **Codex running locally on your Mac** (desktop app or CLI) or **Claude Code**. If Patter is installed elsewhere or you are using a development build, append the JSON from **Connection setup → Copy settings** to the same message. The prompt configures the client you are using, so run it once in each client you want to connect.

```text
Set up Patter's local MCP server in this client on my Mac.

Inspect the existing patter MCP entry first. Use the command and complete args
from any Patter connection JSON I include below, including --socket. Otherwise,
keep an existing patter connection, or use this standard installation if present:
command: /Applications/Patter.app/Contents/MacOS/patter
args: ["--mcp"]
Check that the executable exists. If it is missing, ask me for the connection
settings from Patter instead of guessing a path or installing anything.

For Codex, use `codex mcp add patter -- <command> <args...>` for a new entry.
If the CLI is unavailable or broken, merge the equivalent [mcp_servers.patter]
command and args into my user config.toml (under CODEX_HOME if set, otherwise
~/.codex). For Claude Code, use
`claude mcp add --scope user --transport stdio patter -- <command> <args...>`.
Update only Patter's connection when needed. Preserve other servers, settings,
and existing approval rules. Leave an already-correct entry unchanged.

Remind me to keep Patter running with Settings → Agents → Read entire
library enabled. Leave Patter's permissions unchanged. Verify the configuration,
then call patter_status if the tool is available. Do not read or edit any
conversations during setup. If this session cannot load the new server, tell me
to restart the client or start a new session, then ask it to call patter_status.
Distinguish configuration saved from a successful live tool call.
```

A local coding agent can make the configuration edit, subject to its normal file-access approvals. An ordinary Claude Desktop chat or a cloud coding session may not have access to your Mac's configuration; use the manual instructions below in that case. Registering the server does not install Patter or enable its permissions. Use a build with MCP support; v0.4.0 does not include it.

## Codex

For a standard installation, run:

```sh
codex mcp add patter -- /Applications/Patter.app/Contents/MacOS/patter --mcp
codex mcp list
```

Codex uses a user `config.toml` for MCP connections. If the CLI is unavailable, merge this table into `~/.codex/config.toml` (or `config.toml` under your configured `CODEX_HOME`), preserving all other settings. If the table already exists, update its command and args instead of adding a duplicate:

```toml
[mcp_servers.patter]
command = "/Applications/Patter.app/Contents/MacOS/patter"
args = ["--mcp"]
```

Use Patter’s copied command and all arguments for custom installs and development builds. Patter’s JSON is a source for those values; Codex’s config file uses TOML, so do not paste the JSON directly into it. No OAuth login is needed for this stdio connection.

Restart Codex or open a new local session to load the connection, then ask it to call `patter_status`. In the CLI, `/mcp` shows active servers. A configured entry alone does not prove the app is reachable. See the [official Codex MCP documentation](https://developers.openai.com/codex/mcp/).

## Claude Desktop

Open **Settings → Developer → Edit Config** from Claude’s macOS menu bar. Merge the `patter` entry into the existing `mcpServers` object; preserve other servers and settings. Save, fully quit Claude and reopen it. These are the steps in the [official local MCP guide](https://modelcontextprotocol.io/docs/develop/connect-local-servers).

The file on macOS is `~/Library/Application Support/Claude/claude_desktop_config.json`. For a standard release installation, its Patter entry looks like this:

```json
{
  "mcpServers": {
    "patter": {
      "command": "/Applications/Patter.app/Contents/MacOS/patter",
      "args": ["--mcp"]
    }
  }
}
```

Prefer the configuration copied from Patter: it includes your actual executable and socket paths, which can differ for development builds or custom installs. The example above is for a release build that includes this feature; v0.4.0 does not.

## Claude Code

Register Patter for your user account, across projects:

```sh
claude mcp add --scope user --transport stdio patter -- /Applications/Patter.app/Contents/MacOS/patter --mcp
claude mcp get patter
```

For a development build or a different install location, substitute the command and all arguments from Patter’s **Connection setup**, including `--socket` when present. Use `/mcp` inside Claude Code to check status. See [Claude Code’s MCP reference](https://code.claude.com/docs/en/mcp).

## Check the connection

With Patter open, ask your agent to call `patter_status`. It should return the app version, recording state and backup progress without reading conversation text. Then try:

- “Find my conversations from this week in Patter.”
- “Read the transcript of that conversation.”
- “Show that conversation in Patter.”
- With editing enabled: “Mark that existing action as done.”
- With processing enabled: “Generate an Interview summary for that conversation.”

Tool calls follow the client’s own approval settings as well as Patter’s permission switches. Transcription needs a downloaded model; summary generation needs the configured local model server running.

Other local MCP clients can use the same executable and arguments with **stdio** transport. There is no server URL to enter in a web connector. No Node, Python, cloud service or listening TCP port is needed for the installed integration.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| Agent access is missing in Settings | Use a build with this feature; it is not in v0.4.0. The browser preview cannot serve MCP. |
| Server cannot start | Check the executable path in Patter’s copied settings and fully restart the client. |
| Tools list, but calls say Patter is unavailable | Open Patter on the same Mac and user account. For development builds, copy the matching `--socket` argument. |
| Access or an operation is denied | Enable Read entire library, plus the edit or processing permission needed. Check the client’s own tool permissions too. |
| A write reports a revision conflict | Have the agent reread the conversation and reconsider its edit. Do not retry with a guessed revision. |
| Processing fails | Check model setup, finish recording and wait for any existing job. Use `job_status` for the error. |
| Connection fails after an update or moving the app | Copy the new settings if the path changed, then restart the client connection. |

The helper exits with its client and opens no app window. It never starts Patter or falls back to direct database access. To disconnect all agents, turn off **Read entire library**. You can also remove just the `patter` entry from a client’s configuration.

## Permissions and privacy

Agent access starts off. Permissions are shared by all clients under your macOS account, not per-client credentials. Read access covers the whole Patter library including archived conversations and old versions. This is not a sandbox against other software already running as you. Socket directory and peer ownership are checked; other OS users are refused.

A cloud agent may send returned excerpts to its model provider. Patter's own transcription and summary tools continue to use its configured local models. Nothing about using a local MCP connection makes a cloud model local.

Turning access off rejects subsequent requests. Revoking or changing permissions during processing prevents that result from being saved, but does not interrupt the underlying model computation. A running job continues to hold the app's activity lock until it finishes. Permissions live outside library backups, in `~/.patter-agent/settings.json`; restoring a library never grants agent access on another Mac. Development builds use `.patter-agent-dev` instead.

## Available tools

- `search_conversations`: text, inclusive date range, archived filter, excerpts and pagination.
- `get_conversation`: overview, notes, summary or timestamped transcript; optional historic revision. Text pagination counts Unicode characters; transcript pagination counts segments.
- `conversation_history`: revision numbers, save times and agent attribution.
- `edit_conversation`: replace notes/title or toggle an existing action item. Requires the revision read by the client. It cannot detach recordings or delete history.
- `list_templates`: template names and IDs.
- `start_processing`: transcribe existing audio or generate a summary with an optional template. Requires processing permission and a current revision; only one agent processing job runs at once. Finish recording first.
- `job_status`: running/completed/failed stage and resulting revision/error. Recent jobs are retained for the current app session, up to 100 entries.
- `open_conversation`: request navigation to the conversation in Patter. Unsaved local edits must be resolved before navigation.
- `patter_status`: app version, recording state and sanitized backup progress.

No raw SQL, arbitrary file access, audio download, credential access, deletion, recording control, restore, cloud backup trigger or app-update tool is exposed. Requests are limited to 64 KiB and responses to 512 KiB. Search/history pages max out at 50 rows; transcript pages at 50 segments; text pages at 8,000 characters. A large individual response may require a smaller page.

## Concurrent edits and history

The UI and agent writes both check the revision they started from. If it changed, the write is rejected. Long-running generation uses the same check before saving. Agents must reread and reconsider their edit rather than retry with a guessed revision.

Patter refreshes after agent changes. If you have an unsaved local draft, it stays visible with **Save my draft as a separate conversation**. This preserves both versions without silently merging or overwriting text. Agent-authored saves are marked in version history. The settings activity list covers the current session.

## Development checks

Run `pnpm build`, `pnpm test`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check` and `cargo test --locked --manifest-path src-tauri/Cargo.toml`. The native tests include an official MCP SDK client over the socket bridge, permission denial/revocation, stale writes, paging, history/audio preservation and missing-app errors.

A debug build accepts `--qa-library=/tmp/PATH` for a disposable native test library and private socket. Use a separate Tauri identifier when the installed app is already running. This override is compiled out of release builds. `scripts/mcp-smoke.mjs` exercises the actual stdio executable against a synthetic conversation titled `MCP QA`; its process test expects a deterministic local QA model response. Never run mutation smoke tests against a personal library.
