import { ClaimState, GitHubIssueComment } from "@/lib/types";

const CLAIM_PATTERN = /^\/claim(?:\s+@?([A-Za-z0-9-]+))?/im;
const RELEASE_PATTERN = /^\/release\b/im;

export function inferClaimState(comments: GitHubIssueComment[]): ClaimState {
  let claimedBy: string | null = null;
  let claimedAt: string | null = null;

  for (const comment of comments) {
    const body = comment.body || "";
    if (RELEASE_PATTERN.test(body) && claimedBy === comment.user.login) {
      claimedBy = null;
      claimedAt = null;
      continue;
    }

    const claimMatch = body.match(CLAIM_PATTERN);
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

export function getClaimMessage(username: string) {
  const normalized = username.trim().replace(/^@+/, "");
  return normalized ? `/claim @${normalized}` : "/claim";
}
