const { execFileSync } = require('node:child_process');

const marker = /<!-- android-library-release commit=([a-f0-9]{40}) -->/;
const paths = [
  'src', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
  'LICENSE-MIT', 'LICENSE-APACHE',
  'crates/geocoordinates-ffi', '.github/workflows/release-android.yml',
  '.github/scripts/android-release.cjs', '.mise.toml',
];

// A prefix alone is insufficient: only releases created by this pipeline qualify.
function ownedRelease(release) {
  return release.tag_name.startsWith('android-') && marker.test(release.body || '');
}

function androidReleases(releases) {
  return releases
    .filter(r => !r.draft && ownedRelease(r))
    .sort((a, b) => Date.parse(b.published_at) - Date.parse(a.published_at) || b.id - a.id);
}

async function list(github, context) {
  return github.paginate(github.rest.repos.listReleases, {
    ...context.repo, per_page: 100,
  });
}

async function plan({ github, context, core }, git = args =>
  execFileSync('git', args, { encoding: 'utf8' }).trim()) {
  const releases = androidReleases(await list(github, context));
  const sha = git(['rev-parse', 'HEAD']);
  let changed = true;
  if (releases.length) {
    const previous = releases[0].body.match(marker)[1];
    // Full checkout history is available. Failure to resolve a baseline must fail
    // visibly, rather than silently skip an unpublished change.
    changed = git(['diff', '--name-only', previous, sha, '--', ...paths]) !== '';
  }
  core.setOutput('changed', String(changed));
  core.setOutput('sha', sha);
  core.info(changed ? `Build Android Library from ${sha}` : 'No Android build inputs changed.');
}

async function prune({ github, context, core }) {
  const all = await list(github, context);
  const expired = androidReleases(all).slice(3);
  // Serialized runs guarantee these drafts are abandoned uploads, not an active
  // publication. They never serve as change-detection baselines.
  expired.push(...all.filter(r => r.draft && ownedRelease(r)));
  for (const release of expired) {
    await github.rest.repos.deleteRelease({ ...context.repo, release_id: release.id });
    // Delete only the exact tag belonging to the removed Android release.
    try {
      await github.rest.git.deleteRef({ ...context.repo, ref: `tags/${release.tag_name}` });
    } catch (error) {
      if (error.status !== 404) throw error;
    }
    core.info(`Removed ${release.tag_name}`);
  }
}

module.exports = { androidReleases, plan, prune };
