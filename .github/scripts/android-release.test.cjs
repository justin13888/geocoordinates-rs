const { test } = require('node:test');
const assert = require('node:assert/strict');
const { androidReleases, plan: planImplementation, prune } = require('./android-release.cjs');

const sha = 'a'.repeat(40);
function plan(args, git = () => sha, manifest = '[package]\nversion = "0.14.2"\n') {
  return planImplementation(args, git, manifest);
}
function release(id, overrides = {}) {
  return {
    id, tag_name: `android-test-${id}`, draft: false,
    body: `<!-- android-library-release commit=${sha} -->`,
    published_at: new Date(2026, 0, id).toISOString(), ...overrides,
  };
}

function api(releases) {
  const deleted = [];
  const tags = [];
  const outputs = {};
  return {
    github: {
      paginate: async () => releases,
      rest: {
        repos: {
          listReleases() {}, deleteRelease: async r => deleted.push(r.release_id),
          getLatestRelease: async () => ({ data: { tag_name: 'v0.14.2' } }),
        },
        git: { deleteRef: async r => tags.push(r.ref) },
      },
    },
    context: { repo: { owner: 'owner', repo: 'repo' } },
    core: { info() {}, setOutput: (key, value) => { outputs[key] = value; } },
    deleted, tags, outputs,
  };
}

test('ownership filtering excludes normal releases, drafts and unrelated Android releases', () => {
  const releases = [release(1), release(2), release(3, { draft: true }),
    release(4, { tag_name: 'v0.14.2' }), release(5, { body: 'not ours' })];
  assert.deepEqual(androidReleases(releases).map(r => r.id), [2, 1]);
});

test('first run builds even without any previous release', async () => {
  const args = api([]);
  await plan(args, () => sha);
  assert.equal(args.outputs.changed, 'true');
  assert.equal(args.outputs.sha, sha);
});

test('an already published upstream version is immutable even when the commit changes', async () => {
  const args = api([release(1, { tag_name: 'android-v0.14.2' })]);
  await plan(args, () => 'b'.repeat(40));
  assert.equal(args.outputs.changed, 'false');
  assert.equal(args.outputs.version, '0.14.2');
  assert.equal(args.outputs.tag, 'android-v0.14.2');
});

test('a previous version or failed draft does not block the next release', async () => {
  const args = api([release(1, { tag_name: 'android-v0.14.1' }),
    release(2, { tag_name: 'android-v0.14.2', draft: true })]);
  await plan(args, () => sha);
  assert.equal(args.outputs.changed, 'true');
});

test('wait for synchronization or an official upstream release when versions differ', async () => {
  for (const version of ['0.14.1', '0.15.0']) {
    const args = api([]);
    await plan(args, () => sha, `[package]\nversion = "${version}"\n`);
    assert.equal(args.outputs.changed, 'false');
  }
});

test('invalid versions and upstream API failures fail without inventing a version', async () => {
  await assert.rejects(plan(api([]), () => sha, 'version = "0.15.0-dev"'), /stable crate version/);
  const args = api([]);
  args.github.rest.repos.getLatestRelease = async () => { throw new Error('upstream unavailable'); };
  await assert.rejects(plan(args, () => sha), /upstream unavailable/);
});

test('retention removes older owned releases, abandoned drafts and their exact tags', async () => {
  const args = api([1, 2, 3, 4, 5].map(id => release(id)).concat([
    release(6, { tag_name: 'v0.14.2' }), release(7, { draft: true }),
  ]));
  await prune(args);
  assert.deepEqual(args.deleted, [2, 1, 7]);
  assert.deepEqual(args.tags, ['tags/android-test-2', 'tags/android-test-1', 'tags/android-test-7']);
});

test('retention is a no-op at three releases', async () => {
  const args = api([release(1), release(2), release(3)]);
  await prune(args);
  assert.deepEqual(args.deleted, []);
});

test('retention finds Android releases even behind more than one page of other releases', async () => {
  const others = Array.from({ length: 105 }, (_, id) => release(id, { tag_name: `v${id}` }));
  const args = api(others.concat([1, 2, 3, 4].map(id => release(id))));
  await prune(args);
  assert.deepEqual(args.deleted, [1]);
});

test('a failed release deletion stops retention before deleting its tag', async () => {
  const args = api([1, 2, 3, 4].map(id => release(id)));
  args.github.rest.repos.deleteRelease = async () => { throw new Error('permission denied'); };
  await assert.rejects(prune(args), /permission denied/);
  assert.deepEqual(args.tags, []);
});

test('a missing old tag is tolerated, other deletion failures are surfaced', async () => {
  for (const status of [404, 403]) {
    const args = api([1, 2, 3, 4].map(id => release(id)));
    args.github.rest.git.deleteRef = async () => { throw Object.assign(new Error('delete'), { status }); };
    if (status === 404) await prune(args);
    else await assert.rejects(prune(args), /delete/);
  }
});
