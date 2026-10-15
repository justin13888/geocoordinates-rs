const { execFileSync } = require('node:child_process');
const { readFileSync } = require('node:fs');

const marker = /<!-- android-library-release commit=([a-f0-9]{40}) -->/;

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
  execFileSync('git', args, { encoding: 'utf8' }).trim(),
  manifest = readFileSync('Cargo.toml', 'utf8')) {
  const releases = androidReleases(await list(github, context));
  const sha = git(['rev-parse', 'HEAD']);
  const version = manifest.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!version || !/^\d+\.\d+\.\d+$/.test(version)) throw new Error('Expected a stable crate version');
  const upstream = await github.rest.repos.getLatestRelease({ owner: 'justin13888', repo: 'geocoordinates-rs' });
  const upstreamVersion = upstream.data.tag_name.replace(/^v/, '');
  const tag = `android-v${version}`;
  // Publish each official upstream version once. Development commits retaining
  // the same Cargo version must not create additional, differently numbered releases.
  const changed = version === upstreamVersion && !releases.some(r => r.tag_name === tag);
  core.setOutput('changed', String(changed));
  core.setOutput('sha', sha);
  core.setOutput('version', version);
  core.setOutput('tag', tag);
  core.info(changed ? `Build ${tag} from ${sha}` : `Skip: upstream=${upstreamVersion}, local=${version}; version may already be published.`);
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
