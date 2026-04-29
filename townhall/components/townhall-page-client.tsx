"use client";

import Link from "next/link";
import { useMemo, useState } from "react";
import { BoardClient } from "@/components/board-client";
import { TownhallShell } from "@/components/townhall-shell";
import { BoardIssue } from "@/lib/types";

type Props = {
  repoSlug: string;
  initialIssues: BoardIssue[];
  snapshotGeneratedAt: string;
};

function formatSnapshotDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function TownhallPageClient({ repoSlug, initialIssues, snapshotGeneratedAt }: Props) {
  const [issues] = useState<BoardIssue[]>(initialIssues);
  const loading = false;

  const { unclaimed, claimed, lanes } = useMemo(() => {
    const openCount = issues.filter((issue) => !issue.claim.claimedBy).length;
    return {
      unclaimed: openCount,
      claimed: issues.length - openCount,
      lanes: Array.from(new Set(issues.map((issue) => issue.lane))).sort(),
    };
  }, [issues]);

  return (
    <TownhallShell repoSlug={repoSlug} currentPage="board">
        <section className="hero">
          <div className="hero-eyebrow">town notice board</div>

          <h1 className="hero-title">
            help build
            <span className="gradient"> name not decided city</span>
          </h1>

          <p className="hero-tagline">
            <strong>Maintainers keep the architecture.</strong> The community gets the fun stuff: jobs, restaurants,
            foods, items, map locations, art, frontend panels, docs, and playtesting.
          </p>

          <div className="hero-row">
            <div className="hero-row-stack">
              <div className="pill-row">
                <span className="pill accent">{repoSlug}</span>
                <span className="pill">no auth</span>
                <span className="pill">claim with /claim</span>
                <span className="pill">snapshot: {formatSnapshotDate(snapshotGeneratedAt)}</span>
              </div>

              <div className="cta-row">
                <a className="btn btn-primary" href="#board">
                  browse tasks
                </a>
                <Link className="btn btn-ghost" href="/status">
                  project status
                </Link>
                <a
                  className="btn btn-ghost"
                  href="https://buymeacoffee.com/vedant0200"
                  target="_blank"
                  rel="noreferrer"
                >
                  buy me a coffee
                </a>
                <a
                  className="btn btn-ghost"
                  href={`https://github.com/${repoSlug}/blob/main/docs/community-contributions.md`}
                  target="_blank"
                  rel="noreferrer"
                >
                  contribution guide
                </a>
              </div>
            </div>

            <div className="stats-card">
              <div className="panel-title">board stats</div>
              <div className="stats-grid">
                <div className="stat-tile">
                  <div className="stat-value">{loading ? "..." : issues.length}</div>
                  <div className="stat-label">tasks</div>
                </div>
                <div className="stat-tile">
                  <div className="stat-value">{loading ? "..." : unclaimed}</div>
                  <div className="stat-label">open</div>
                </div>
                <div className="stat-tile">
                  <div className="stat-value">{loading ? "..." : lanes.length}</div>
                  <div className="stat-label">lanes</div>
                </div>
              </div>
              <div className="you-claims">
                {loading ? (
                  <span>loading board data...</span>
                ) : (
                  <>
                    claimed now: <strong>{claimed}</strong> / <strong>{issues.length}</strong>
                  </>
                )}
              </div>
            </div>
          </div>
        </section>

        <div className="notice">
          <span className="notice-icon" aria-hidden>
            !
          </span>
          <span>
            This board is served from a build-time snapshot so GitHub Pages does not get wrecked by public API rate limits.
            Only community issues show up here, and anything labeled <code>architecture-sensitive</code> or
            <code> maintainer-only</code> stays off the board.
          </span>
        </div>

        <section id="board">
          <BoardClient issues={issues} repoSlug={repoSlug} loading={loading} />
        </section>
    </TownhallShell>
  );
}
