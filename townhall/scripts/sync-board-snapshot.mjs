import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, "..");
const outputPath = path.join(rootDir, "data", "board-snapshot.json");

const DEFAULT_REPO_OWNER = process.env.NEXT_PUBLIC_GITHUB_REPO_OWNER || process.env.GITHUB_REPO_OWNER || "Vedant020000";
const DEFAULT_REPO_NAME = process.env.NEXT_PUBLIC_GITHUB_REPO_NAME || process.env.GITHUB_REPO_NAME || "letta-city-sim";
const GITHUB_API_BASE = "https://api.github.com";
const token = process.env.GITHUB_TOKEN || "";

const COMMUNITY_GATE_LABELS = new Set(["community", "help wanted", "good first issue"]);

function buildHeaders() {
  return {
    Accept: "application/vnd.github+json",
    "User-Agent": "townhall-board-sync",
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
  };
}

async function githubFetch(url) {
  const response = await fetch(url, {
    headers: buildHeaders(),
  });

  if (!response.ok) {
    const body = await response.text();
    throw new Error(`GitHub request failed (${response.status}): ${body || response.statusText}`);
  }

  return response.json();
}

function getLabelNames(labels) {
  return labels.map((label) => label.name.toLowerCase());
}

function isCommunityVisible(labels) {
  const names = getLabelNames(labels);
  if (names.includes("architecture-sensitive") || names.includes("maintainer-only")) {
    return false;
  }

  return names.some((name) => COMMUNITY_GATE_LABELS.has(name));
}

function deriveLane(labels) {
  const names = getLabelNames(labels);
  if (names.includes("art")) return "art";
  if (names.includes("content")) return "content";
  if (names.includes("playtest")) return "playtest";
  if (names.includes("docs")) return "docs";
  if (names.includes("frontend")) return "frontend";
  if (names.includes("cli")) return "cli";
  if (names.includes("backend") || names.includes("api")) return "backend";
  return "general";
}

function deriveDifficulty(labels) {
  const names = getLabelNames(labels);
  if (names.includes("good first issue")) return "good first issue";
  if (names.includes("help wanted")) return "help wanted";
  return "open";
}

function inferClaimState(comments) {
  const claimPattern = /^\/claim(?:\s+@?([A-Za-z0-9-]+))?/im;
  const releasePattern = /^\/release\b/im;

  let claimedBy = null;
  let claimedAt = null;

  for (const comment of comments) {
    const body = comment.body || "";
    if (releasePattern.test(body) && claimedBy === comment.user.login) {
      claimedBy = null;
      claimedAt = null;
      continue;
    }

    const claimMatch = body.match(claimPattern);
    if (claimMatch) {
      claimedBy = comment.user.login;
      claimedAt = comment.created_at;
    }
  }

  return {
    claimedBy,
    claimedAt,
    claimCommentUrl: null,
  };
}

function sortIssues(a, b) {
  if (a.claim.claimedBy && !b.claim.claimedBy) return 1;
  if (!a.claim.claimedBy && b.claim.claimedBy) return -1;
  return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
}

async function main() {
  const repoSlug = `${DEFAULT_REPO_OWNER}/${DEFAULT_REPO_NAME}`;
  const issuesUrl = `${GITHUB_API_BASE}/repos/${DEFAULT_REPO_OWNER}/${DEFAULT_REPO_NAME}/issues?state=open&per_page=100&sort=updated&direction=desc`;
  const issues = await githubFetch(issuesUrl);
  const visibleIssues = issues.filter((issue) => !issue.pull_request).filter((issue) => isCommunityVisible(issue.labels));

  const hydratedIssues = await Promise.all(
    visibleIssues.map(async (issue) => {
      const comments = await githubFetch(issue.comments_url);
      return {
        ...issue,
        claim: inferClaimState(comments),
        lane: deriveLane(issue.labels),
        difficulty: deriveDifficulty(issue.labels),
        isCommunityVisible: true,
      };
    }),
  );

  const snapshot = {
    generated_at: new Date().toISOString(),
    repo_slug: repoSlug,
    issues: hydratedIssues.sort(sortIssues),
  };

  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await fs.writeFile(outputPath, `${JSON.stringify(snapshot, null, 2)}\n`, "utf8");

  console.log(`Wrote board snapshot for ${repoSlug} to ${outputPath}`);
  console.log(`Included ${snapshot.issues.length} community-visible issues.`);
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
