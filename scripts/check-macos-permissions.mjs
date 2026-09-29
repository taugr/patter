import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { resolve } from 'node:path';

// Inspect the shipped signature, not just the source configuration: a plist
// alongside the executable does not grant a Hardened Runtime capability.
const bundle = resolve(process.argv[2] || 'src-tauri/target/release/bundle/macos/Patter.app');
const exec = (command, args, input) => execFileSync(command, args, {
  encoding: 'utf8', input, stdio: ['pipe', 'pipe', 'pipe'],
});
const plist = (xml) => JSON.parse(exec('/usr/bin/plutil', ['-convert', 'json', '-o', '-', '-'], xml));

exec('/usr/bin/codesign', ['--verify', '--deep', '--strict', bundle]);
const signed = exec('/usr/bin/codesign', ['--display', '--entitlements', '-', '--xml', bundle]);
assert(signed.trim(), 'Bundle has no signed entitlements; Calendar consent will fail under Hardened Runtime');
assert.equal(plist(signed)['com.apple.security.personal-information.calendars'], true,
  'Bundle is missing the signed Calendar entitlement');
const info = JSON.parse(exec('/usr/bin/plutil', ['-convert', 'json', '-o', '-', `${bundle}/Contents/Info.plist`]));
assert.equal(typeof info.NSCalendarsFullAccessUsageDescription, 'string', 'Missing Calendar consent description');
assert(info.NSCalendarsFullAccessUsageDescription.trim(), 'Empty Calendar consent description');
console.log('Verified signed Calendar entitlement and full-access consent description.');
