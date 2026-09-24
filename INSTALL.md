# Install and update Patter

Patter is a personal preview for **Apple Silicon (M1 or newer), macOS 15+**.

## Install once

1. Download the DMG from [the latest release](https://github.com/taugr/patter/releases/latest).
2. Open it and drag Patter into Applications. Eject the DMG.
3. Open Patter from Applications. This build uses free ad-hoc signing, without an Apple Developer certificate or notarization. If macOS blocks it, use the app-specific **Open Anyway** option in System Settings → Privacy & Security. Do not disable Gatekeeper globally.
4. Configure local models and Calendar in Settings as described in the README.

An existing 0.1.0 development build has no updater: replace it manually once. App replacement keeps `~/Library/Application Support/gr.tau.patter/` intact. Each Mac has its own library; updates do not synchronize meetings.

## Update

Patter checks quietly when the library opens. An available update appears in Settings → Updates; the Patter menu also has **Check for Updates**. Choose **Install and restart** when ready. Nothing installs or restarts without that click.

Updates are downloaded over HTTPS, verified against Patter's embedded public key, and checked against their signed release version. The native activity lock refuses updates while recording, processing, importing, saving, or backing up. Pending frontend edits are flushed first. Failed downloads, verification or backups leave the app installed and show a retryable message.

A consistent database snapshot is retained in the library's `backups/` directory before replacement and schema migrations. It contains conversations, transcripts, versions and settings; original audio stays in `recordings/` and is never modified by the updater. For an independent copy including audio, use Settings → Library → Back up library.

Ad-hoc signing can cause macOS to request app approval or capture/calendar permissions again after an update. Permission continuity has not been proven on a second Mac. Live audio capture and real model inference still need supervised reliability testing.

## Publish a new version

Commit and push normal changes to `main`; GitHub runs checks without publishing an app update. When you want to ship:

```sh
pnpm release:patch       # or pnpm release:minor
```

This command requires a clean, fully pushed `main`, increments all version files, creates a release commit and tag, and **pushes both**. A version tag runs tests, builds the ad-hoc DMG and signed updater archive, validates signatures/checksums, uploads a complete draft, then publishes it. Follow [Actions](https://github.com/taugr/patter/actions). An ordinary source push alone does not update installed apps.

`PATTER_AUTO_PUBLISH=true` enables publication after all gates pass. Set it to false to hold future builds as drafts. Published artifacts are immutable; a fix needs a higher version. Concurrent release jobs serialize, and older versions cannot supersede a newer stable release. During initial upgrade proof, `PATTER_TEST_FEED` may override the feed in a bootstrap test build; clear it before stable publication.

## Signing key and recovery

The updater's free signing key is separate from Apple code signing. GitHub Actions secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` are available only to the release build step. Never put either in source, logs or an app bundle. An encrypted local copy is kept in the ignored `.secrets/` directory; preserve it and its password separately in your own secure backup. GitHub secrets cannot be downloaded later.

If the key is lost, existing clients cannot trust a new signing identity automatically: distribute a manual replacement or use a deliberate key migration while the old key still exists. No automatic database rollback is promised. If a release fails after a schema migration, prefer a corrected newer release. For manual recovery, quit Patter, copy the entire current library somewhere safe, and restore a compatible database snapshot together with its original recordings. Do not overwrite or delete the only copy. Backups currently contain absolute recording paths; moving between accounts requires path migration.

## Validation limits

Automated gates cover frontend persistence/search, native activity exclusion, immutable versions/audio references, schema rejection, failed migration rollback and database backup restoration. Signature/packaging checks validate release assets before publication. Real capture, Calendar access, interrupted downloads, actual disk exhaustion, and permissions across two physical Macs remain separate verification tasks.
