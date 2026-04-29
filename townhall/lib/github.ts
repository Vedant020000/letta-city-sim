import { GitHubIssue, GitHubIssueComment } from "@/lib/types";

const DEFAULT_REPO_OWNER = "Vedant020000";
const DEFAULT_REPO_NAME = "letta-city-sim";
const GITHUB_API_BASE = "https://api.github.com";

function getRepoConfig() {
  const isBrowser = typeof window !== "undefined";
  return {
    owner:
      process.env.NEXT_PUBLIC_GITHUB_REPO_OWNER ||
      process.env.GITHUB_REPO_OWNER ||
      DEFAULT_REPO_OWNER,
    repo:
      process.env.NEXT_PUBLIC_GITHUB_REPO_NAME ||
      process.env.GITHUB_REPO_NAME ||
      DEFAULT_REPO_NAME,
    token: isBrowser ? "" : process.env.GITHUB_TOKEN || "",
  };
}

function buildHeaders() {
  const { token } = getRepoConfig();
  return {
    Accept: "application/vnd.github+json",
    "User-Agent": "townhall-board",
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
  };
}

async function githubFetch<T>(url: string): Promise<T> {
  const requestInit: RequestInit = {
    headers: buildHeaders(),
  };

  if (typeof window === "undefined") {
    Object.assign(requestInit, { next: { revalidate: 300 } });
  }

  const response = await fetch(url, requestInit);

  if (!response.ok) {
    const body = await response.text();
    throw new Error(`GitHub request failed (${response.status}): ${body || response.statusText}`);
  }

  return response.json() as Promise<T>;
}

export function getRepoSlug() {
  const { owner, repo } = getRepoConfig();
  return `${owner}/${repo}`;
}

export async function fetchOpenIssues(): Promise<GitHubIssue[]> {
  const { owner, repo } = getRepoConfig();
  const url = `${GITHUB_API_BASE}/repos/${owner}/${repo}/issues?state=open&per_page=100&sort=updated&direction=desc`;
  const issues = await githubFetch<GitHubIssue[]>(url);
  return issues.filter((issue) => !issue.pull_request);
}

export async function fetchIssueComments(commentsUrl: string): Promise<GitHubIssueComment[]> {
  return githubFetch<GitHubIssueComment[]>(commentsUrl);
}
