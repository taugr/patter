import { readFileSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
const read = path => readFileSync(path, 'utf8');
const pkg = JSON.parse(read('package.json'));
const config = JSON.parse(read('src-tauri/tauri.conf.json'));
const cargo = read('src-tauri/Cargo.toml');
const lock = read('src-tauri/Cargo.lock');
assert.equal(config.version, pkg.version, 'Tauri/package versions differ');
assert.equal(cargo.match(/^version = "([^"]+)"/m)?.[1], pkg.version, 'Cargo/package versions differ');
assert.equal(lock.match(/name = "patter"\nversion = "([^"]+)"/)?.[1], pkg.version, 'Cargo lock/package versions differ');
const command = process.argv[2];
if (command === '--check') {
  const tag = process.env.GITHUB_REF_TYPE === 'tag' ? process.env.GITHUB_REF_NAME : process.argv[3];
  if (tag) assert.equal(tag, `v${pkg.version}`, 'Release tag/version mismatch');
  console.log(`Versions agree: ${pkg.version}`);
} else {
  assert(['patch', 'minor'].includes(command), 'Use --check, patch or minor');
  const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim();
  assert.equal(git('status', '--porcelain'), '', 'Commit all changes before releasing');
  assert.equal(git('branch', '--show-current'), 'main', 'Release from main');
  git('fetch', 'origin', 'main');
  assert.equal(git('rev-parse', 'HEAD'), git('rev-parse', 'origin/main'), 'Push main and pull remote changes first');
  const parts = pkg.version.split('.').map(Number);
  if (command === 'patch') parts[2]++; else { parts[1]++; parts[2] = 0; }
  const next = parts.join('.');
  pkg.version = next; config.version = next;
  writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
  writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(config, null, 2) + '\n');
  writeFileSync('src-tauri/Cargo.toml', cargo.replace(/^version = "[^"]+"/m, `version = "${next}"`));
  writeFileSync('src-tauri/Cargo.lock', lock.replace(/name = "patter"\nversion = "[^"]+"/, `name = "patter"\nversion = "${next}"`));
  git('add', 'package.json', 'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock');
  git('commit', '-m', `chore: release v${next}`);
  git('tag', `v${next}`);
  git('push', '--atomic', 'origin', 'main', `v${next}`);
  console.log(`Pushed v${next}. Follow the Release workflow on GitHub.`);
}
