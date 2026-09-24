import {execFileSync} from 'node:child_process';
import {readFileSync,mkdtempSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
const {version}=JSON.parse(readFileSync('package.json','utf8'));
const gh=(...args)=>execFileSync('gh',args,{encoding:'utf8'});
const repo=process.env.GITHUB_REPOSITORY || 'taugr/patter';
const releases=JSON.parse(gh('api','--paginate','--slurp',`repos/${repo}/releases`)).flat();
const release=releases.find(r=>r.tag_name===`v${version}`);
assert(release?.draft,'Only a draft release can be published; published assets are immutable');
const compare=(a,b)=>{const x=a.replace(/^v/,'').split('.').map(Number),y=b.replace(/^v/,'').split('.').map(Number);return x[0]-y[0]||x[1]-y[1]||x[2]-y[2]};
assert(!releases.some(r=>!r.draft && !r.prerelease && compare(r.tag_name,version)>=0),'A same or newer stable release already exists');
const assets=release.assets.map(a=>a.name);
for(const name of [`Patter_${version}_aarch64.dmg`,'Patter.app.tar.gz','Patter.app.tar.gz.sig','latest.json','SHA256SUMS']) assert(assets.includes(name),`Missing ${name}`);
const downloaded=mkdtempSync(join(tmpdir(),'patter-release-'));
try {
  gh('release','download',`v${version}`,'--repo',repo,'--dir',downloaded);
  const sums=readFileSync('release-artifacts/SHA256SUMS','utf8');
  assert.equal(readFileSync(join(downloaded,'SHA256SUMS'),'utf8'),sums,'Uploaded checksums differ');
  for(const line of sums.trim().split('\n')) {
    const [hash,name]=line.split('  ');
    assert.equal(createHash('sha256').update(readFileSync(join(downloaded,name))).digest('hex'),hash,`Uploaded ${name} checksum mismatch`);
  }
} finally { rmSync(downloaded,{recursive:true,force:true}); }
gh('release','edit',`v${version}`,'--repo',repo,'--draft=false','--latest');
console.log(`Published v${version}`);
