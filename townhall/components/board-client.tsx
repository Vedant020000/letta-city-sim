"use client";

import { useEffect, useMemo, useState } from "react";
import { getClaimMessage } from "@/lib/claims";
import { BoardIssue } from "@/lib/types";

type Props = {
  issues: BoardIssue[];
  repoSlug: string;
  loading?: boolean;
};

const USERNAME_KEY = "townhall-github-username";

const LANE_COLORS: Record<string, string> = {
  art: "var(--lane-art)",
  content: "var(--lane-content)",
  frontend: "var(--lane-frontend)",
  backend: "var(--lane-backend)",
  cli: "var(--lane-cli)",
  docs: "var(--lane-docs)",
  playtest: "var(--lane-playtest)",
  general: "var(--lane-general)",
};

function laneColor(lane: string) {
  return LANE_COLORS[lane] || LANE_COLORS.general;
}

function formatDate(value: string) {
  const d = new Date(value);
  const days = Math.floor((Date.now() - d.getTime()) / (1000 * 60 * 60 * 24));
  if (days <= 0) return "today";
  if (days === 1) return "yesterday";
  if (days < 30) return `${days}d ago`;
  if (days < 365) return `${Math.floor(days / 30)}mo ago`;
  return `${Math.floor(days / 365)}y ago`;
}

function IssueCard({
  issue,
  repoSlug,
  viewerUsername,
  activeClaimCount,
}: {
  issue: BoardIssue;
  repoSlug: string;
  viewerUsername: string;
  activeClaimCount: number;
}) {
  const trimmedBody = (issue.body || "No description yet. Open the GitHub issue for details.").trim();
  const preview = trimmedBody.length > 220 ? `${trimmedBody.slice(0, 220).trim()}...` : trimmedBody;
  const claimMessage = getClaimMessage(viewerUsername);
  const claimDisabled = Boolean(
    viewerUsername && activeClaimCount >= 1 && issue.claim.claimedBy?.toLowerCase() !== viewerUsername,
  );
  const [copied, setCopied] = useState(false);

  const copyClaim = async () => {
    try {
      await navigator.clipboard.writeText(claimMessage);
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    } catch {
      // ignore clipboard failures
    }
  };

  return (
    <article className="issue-card" style={{ ["--lane-color" as string]: laneColor(issue.lane) }}>
      <div className="issue-head">
        <div>
          <div className="issue-meta">
            <span>#{issue.number}</span>
            <span className="meta-dot" />
            <span className="lane-tag">{issue.lane}</span>
          </div>
          <h3 className="issue-title">{issue.title}</h3>
        </div>

        <span className={`status-badge ${issue.claim.claimedBy ? "claimed" : "unclaimed"}`}>
          {issue.claim.claimedBy ? `claimed by @${issue.claim.claimedBy}` : "unclaimed"}
        </span>
      </div>

      {issue.labels.length > 0 ? (
        <div className="label-row">
          {issue.labels.map((label) => (
            <span className="label" key={label.id}>
              {label.name}
            </span>
          ))}
        </div>
      ) : null}

      <p className="issue-body">{preview}</p>

      <div className="meta-row">
        <span>opened by @{issue.user.login}</span>
        <span className="meta-dot" />
        <span>updated {formatDate(issue.updated_at)}</span>
        {issue.claim.claimedAt ? (
          <>
            <span className="meta-dot" />
            <span>claimed {formatDate(issue.claim.claimedAt)}</span>
          </>
        ) : null}
      </div>

      <div className="action-row">
        <a
          className={`btn btn-primary ${claimDisabled ? "disabled" : ""}`}
          href={issue.html_url}
          target="_blank"
          rel="noreferrer"
          aria-disabled={claimDisabled}
          onClick={(event) => {
            if (claimDisabled) event.preventDefault();
          }}
          title={
            claimDisabled
              ? "You already have an active claim. Release it before claiming another."
              : `Claim ${repoSlug}#${issue.number} on GitHub`
          }
        >
          {claimDisabled ? "already holding one" : issue.claim.claimedBy ? "open issue" : "claim on github"}
        </a>

        <button type="button" className="btn" onClick={copyClaim} title="Copy the comment to paste on GitHub">
          {copied ? "copied" : `copy ${claimMessage}`}
        </button>

        <a className="btn btn-ghost" href={issue.html_url} target="_blank" rel="noreferrer">
          view issue
        </a>
      </div>
    </article>
  );
}

export function BoardClient({ issues, repoSlug, loading = false }: Props) {
  const [viewerUsername, setViewerUsername] = useState("");
  const [laneFilter, setLaneFilter] = useState("all");
  const [statusFilter, setStatusFilter] = useState("all");
  const [search, setSearch] = useState("");

  useEffect(() => {
    const saved = window.localStorage.getItem(USERNAME_KEY);
    if (saved) setViewerUsername(saved);
  }, []);

  const normalizedViewer = viewerUsername.trim().replace(/^@+/, "").toLowerCase();
  const activeClaimCount = issues.filter((issue) => issue.claim.claimedBy?.toLowerCase() === normalizedViewer).length;

  const lanes = useMemo(
    () => ["all", ...Array.from(new Set(issues.map((issue) => issue.lane))).sort()],
    [issues],
  );

  const filtered = useMemo(() => {
    const query = search.trim().toLowerCase();

    return issues.filter((issue) => {
      if (laneFilter !== "all" && issue.lane !== laneFilter) return false;
      if (statusFilter === "claimed" && !issue.claim.claimedBy) return false;
      if (statusFilter === "unclaimed" && issue.claim.claimedBy) return false;
      if (!query) return true;

      const haystack = [issue.title, issue.body || "", ...issue.labels.map((label) => label.name)].join(" ").toLowerCase();
      return haystack.includes(query);
    });
  }, [issues, laneFilter, statusFilter, search]);

  return (
    <div className="board-grid">
      <aside className="sidebar">
        <section className="panel">
          <div className="panel-title">claim handle</div>
          <div className="field">
            <label htmlFor="github-username">your github username</label>
            <input
              id="github-username"
              value={viewerUsername}
              placeholder="vedant020000"
              spellCheck={false}
              autoComplete="off"
              onChange={(event) => {
                const value = event.target.value;
                setViewerUsername(value);
                window.localStorage.setItem(USERNAME_KEY, value);
              }}
            />
          </div>

          <div className="you-claims">
            active claims for you: <strong>{activeClaimCount}</strong>
          </div>
        </section>

        <section className="panel">
          <div className="panel-title">filters</div>

          <div className="field">
            <label htmlFor="lane-filter">lane</label>
            <select id="lane-filter" value={laneFilter} onChange={(event) => setLaneFilter(event.target.value)}>
              {lanes.map((lane) => (
                <option key={lane} value={lane}>
                  {lane}
                </option>
              ))}
            </select>
          </div>

          <div className="field">
            <label htmlFor="status-filter">status</label>
            <select id="status-filter" value={statusFilter} onChange={(event) => setStatusFilter(event.target.value)}>
              <option value="all">all</option>
              <option value="unclaimed">unclaimed</option>
              <option value="claimed">claimed</option>
            </select>
          </div>

          <div className="field">
            <label htmlFor="search">search</label>
            <input id="search" value={search} placeholder="jobs, food, art, docs" onChange={(event) => setSearch(event.target.value)} />
          </div>
        </section>

        <section className="panel">
          <div className="panel-title">house rules</div>
          <ul className="rules-list">
            <li>Only community-labeled issues appear here.</li>
            <li>Architectural changes stay maintainer-owned.</li>
            <li>Claim by commenting <code>/claim</code>.</li>
            <li>Release by commenting <code>/release</code>.</li>
            <li>Try to hold one active task at a time.</li>
          </ul>
        </section>
      </aside>

      <section className="issues">
        <div className="section-head">
          <h2>open tasks</h2>
          <span className="count">
            {loading ? "loading..." : `${filtered.length} of ${issues.length}`}
          </span>
        </div>

        {loading ? (
          <div className="empty">
            <span className="empty-icon" aria-hidden>
              ...
            </span>
            <h3>loading tasks</h3>
            <p>Fetching community issues from GitHub...</p>
          </div>
        ) : filtered.length === 0 ? (
          <div className="empty">
            <span className="empty-icon" aria-hidden>
              []
            </span>
            <h3>no matching tasks</h3>
            <p>
              Add labels like <code>community</code>, <code>help wanted</code>, or <code>good first issue</code> in <strong>{repoSlug}</strong>.
            </p>
          </div>
        ) : (
          filtered.map((issue) => (
            <IssueCard
              key={issue.id}
              issue={issue}
              repoSlug={repoSlug}
              viewerUsername={normalizedViewer}
              activeClaimCount={activeClaimCount}
            />
          ))
        )}
      </section>
    </div>
  );
}
