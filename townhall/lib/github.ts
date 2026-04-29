const DEFAULT_REPO_OWNER = "Vedant020000";
const DEFAULT_REPO_NAME = "letta-city-sim";

function getRepoConfig() {
  return {
    owner:
      process.env.NEXT_PUBLIC_GITHUB_REPO_OWNER ||
      process.env.GITHUB_REPO_OWNER ||
      DEFAULT_REPO_OWNER,
    repo:
      process.env.NEXT_PUBLIC_GITHUB_REPO_NAME ||
      process.env.GITHUB_REPO_NAME ||
      DEFAULT_REPO_NAME,
  };
}

export function getRepoSlug() {
  const { owner, repo } = getRepoConfig();
  return `${owner}/${repo}`;
}
