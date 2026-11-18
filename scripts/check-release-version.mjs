import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const metadata = JSON.parse(execFileSync('cargo', [
  'metadata', '--no-deps', '--format-version', '1', '--locked',
], { cwd: root, encoding: 'utf8' }));
const members = metadata.packages.filter(pkg => metadata.workspace_members.includes(pkg.id));
const version = members.find(pkg => pkg.name === 'veil')?.version;
assert.ok(version, 'veil package missing from workspace');
for (const pkg of members) {
  assert.equal(pkg.version, version, `${pkg.name} version differs from veil`);
}

const readJson = path => JSON.parse(readFileSync(new URL(path, import.meta.url), 'utf8'));
const ui = readJson('../ui/package.json');
const lock = readJson('../ui/package-lock.json');
for (const [label, actual] of [
  ['ui/package.json', ui.version],
  ['ui/package-lock.json', lock.version],
  ['ui/package-lock.json root package', lock.packages[''].version],
]) {
  assert.equal(actual, version, `${label} version differs from veil`);
}

if (process.env.GITHUB_REF_TYPE === 'tag') {
  assert.equal(process.env.GITHUB_REF_NAME, `v${version}`,
    'release tag must match the package version; bump manifests and lockfiles before tagging');
}
console.log(`Release versions consistent: ${version}`);
