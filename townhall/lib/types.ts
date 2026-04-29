export type GitHubLabel = {
  id: number;
  name: string;
  color: string;
  description?: string | null;
};

export type GitHubIssue = {
  id: number;
  number: number;
  title: string;
  body: string | null;
  html_url: string;
  comments_url: string;
  created_at: string;
  updated_at: string;
  labels: GitHubLabel[];
  user: {
    login: string;
  };
  pull_request?: unknown;
};

export type GitHubIssueComment = {
  id: number;
  body: string;
  created_at: string;
  user: {
    login: string;
  };
};

export type ClaimState = {
  claimedBy: string | null;
  claimedAt: string | null;
  claimCommentUrl: string | null;
};

export type BoardIssue = GitHubIssue & {
  claim: ClaimState;
  lane: string;
  difficulty: string;
  isCommunityVisible: boolean;
};
