# Local agent access

## Scope

Ship an opt-in local MCP server for the Mac app. Patter stays the sole owner of its live library and jobs. The installed executable's `--mcp` mode speaks MCP stdio without a webview; it forwards bounded requests through an owner-only Unix socket to the running app. No HTTP listener, hosted service, Node or Python runtime is needed. Agent access defaults off, with independent editing and local processing permissions. Disable takes effect on subsequent calls and before a queued/generated result is saved.

Initial tools: search conversations (date/text, paginated excerpts), read sections and timestamped transcript pages, version history, patch notes/title/action completion with an expected revision, list summary templates, start local transcription or summarization, poll jobs, open a conversation, and read sanitized recording/backup status. No deletion, raw SQL, arbitrary filesystem access, credentials, recording control, restore, or update installation tools.

## Implementation

1. Extract shared processing services. Check source revisions before saving generated results. Keep permanent versions and original recordings.
2. Add Rust MCP stdio transport using the official SDK and a private socket bridge. Validate inputs and permissions in the app, regardless of what the client advertises. Bound message sizes, search/read pages, concurrent clients and jobs. Jobs hold the existing activity lock and expose progress/status.
3. Add opt-in Settings → Agent access with copyable client configuration, independent write/processing switches, privacy explanation and recent agent activity. Configuration is local to this Mac and is not restored from backups as granted permissions.
4. Notify the UI after mutations. Preserve local drafts and present a conflict recovery action instead of silently replacing them. UI writes use expected revisions too.
5. Document installation, permissions, startup/closed-app behavior, local versus cloud model privacy, and one shared permission policy for all clients running under the same macOS account.

## Validation

- Unit tests: denied access, read-only enforcement, bounded reads, input validation, stale writes, immutable history/audio, processing conflict/revocation, job contention and activity exclusion.
- Protocol tests with an actual MCP SDK client: initialize, list tools, successful/error calls, stdio bridge and missing app.
- Frontend tests: stale-save handling and local-draft preservation.
- Production frontend build, Rust formatting/full tests, packaged app signature and stdio smoke test.
- Render Settings and the draft-conflict flow in the Codex browser; check native settings and UI updates against a disposable library if available.

The initial implementation was validated locally. Release v0.5.0 was subsequently authorized on 28 September 2026. Existing recordings and personal cloud configuration must remain untouched during QA.

## Completed validation — 28 September 2026

- Production frontend build, 7 frontend tests, 44 Rust tests, Rust formatting and whitespace checks passed.
- Official Rust MCP client initialized, discovered all nine tools, read a transcript through the private socket, and handled a missing app.
- A separately identified, ad-hoc-signed debug app used `/tmp/patter-mcp-qa`; its stdio executable passed disabled/read-only/edit/processing smoke checks. A deterministic loopback model verified the summary pipeline, template selection, job polling and duplicate-job rejection.
- Native settings permissions persisted, the client-configuration copy button succeeded, and disabling access denied the next call. Native notes refreshed after agent edits; history showed agent attribution. Stale local-save recovery preserved both notes and all original audio references. The recording SHA-256 remained unchanged.
- Unit tests covered revocation/regrant during generation, source-revision changes, private socket permissions and host restart/exclusion. Native QA found a host-lock integration error, now fixed and covered by the host lifecycle test. An existing OAuth callback test's nonblocking accept race was also fixed in the fixture.
- Codex in-app browser: meaningful page/settings content, no framework overlay or console warnings/errors, and no horizontal overflow at a 760px viewport. Native settings and draft recovery were inspected visually.

Limits: no global Claude configuration was changed and no cloud model received meeting data. Summary QA used a local fixture model; real inference quality and long-running transcription are not newly certified by these integration checks. The debug library intentionally sits outside the production audio asset scope, so its audio was checked by hash rather than playback. These are local implementation checks; release CI separately validates published artifacts. The user explicitly chose to skip the actual Codex/Claude client connection test for v0.5.0.
