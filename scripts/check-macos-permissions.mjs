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

function entitlements(path) {
  exec('/usr/bin/codesign', ['--verify', '--deep', '--strict', path]);
  const signed = exec('/usr/bin/codesign', ['--display', '--entitlements', '-', '--xml', path]);
  assert(signed.trim(), `${path} has no signed entitlements`);
  return plist(signed);
}
function description(info, key, label) {
  assert.equal(typeof info[key], 'string', `${label}: missing ${key}`);
  assert(info[key].trim(), `${label}: empty ${key}`);
}

const signed = entitlements(bundle);
assert.equal(signed['com.apple.security.personal-information.calendars'], true,
  'App is missing the signed Calendar entitlement');
assert.equal(signed['com.apple.security.device.audio-input'], true,
  'App is missing the signed Audio Input entitlement');
const info = JSON.parse(exec('/usr/bin/plutil', ['-convert', 'json', '-o', '-', `${bundle}/Contents/Info.plist`]));
description(info, 'NSCalendarsFullAccessUsageDescription', 'App');

// Resources keep the helper's own signature; checking the app alone misses it.
const helper = `${bundle}/Contents/Resources/resources/patter-native`;
assert.equal(entitlements(helper)['com.apple.security.device.audio-input'], true,
  'Recording helper is missing the signed Audio Input entitlement');
const embedded = exec('/usr/bin/otool', ['-P', helper]);
const start = embedded.indexOf('<?xml');
const end = embedded.indexOf('</plist>', start);
assert(start >= 0 && end > start, 'Recording helper has no embedded Info.plist');
const helperInfo = plist(embedded.slice(start, end + '</plist>'.length));
for (const [label, metadata] of [['App', info], ['Recording helper', helperInfo]]) {
  description(metadata, 'NSMicrophoneUsageDescription', label);
  description(metadata, 'NSScreenCaptureUsageDescription', label);
}
console.log('Verified app/helper signatures, Calendar and Audio Input entitlements, and Calendar/Microphone/Screen Capture consent descriptions.');
