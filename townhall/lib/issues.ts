import { inferClaimState } from "@/lib/claims";
import { fetchIssueComments, fetchOpenIssues } from "@/lib/github";
import { BoardIssue, GitHubLabel } from "@/lib/types";

const COMMUNITY_GATE_LABELS = new Set(["community", "help wanted", "good first issue"]);

function getLabelNames(labels: GitHubLabel[]) {
  return labels.map((label) => label.name.toLowerCase());
}

function isCommunityVisible(labels: GitHubLabel[]) {
  const names = getLabelNames(labels);
  if (names.includes("architecture-sensitive") || names.includes("maintainer-only")) {
    return false;
  }

  return names.some((name) => COMMUNITY_GATE_LABELS.has(name));
}

function deriveLane(labels: GitHubLabel[]) {
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

function deriveDifficulty(labels: GitHubLabel[]) {
  const names = getLabelNames(labels);
  if (names.includes("good first issue")) return "good first issue";
  if (names.includes("help wanted")) return "help wanted";
  return "open";
}

function sortIssues(a: BoardIssue, b: BoardIssue) {
  if (a.claim.claimedBy && !b.claim.claimedBy) return 1;
  if (!a.claim.claimedBy && b.claim.claimedBy) return -1;
  return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
}

export async function getBoardIssues(): Promise<BoardIssue[]> {
  const issues = await fetchOpenIssues();
  const visibleIssues = issues.filter((issue) => isCommunityVisible(issue.labels));

  const withClaims = await Promise.all(
    visibleIssues.map(async (issue) => {
      const comments = await fetchIssueComments(issue.comments_url);
      return {
        ...issue,
        claim: inferClaimState(comments),
        lane: deriveLane(issue.labels),
        difficulty: deriveDifficulty(issue.labels),
        isCommunityVisible: true,
      } satisfies BoardIssue;
    }),
  );

  return withClaims.sort(sortIssues);
}
