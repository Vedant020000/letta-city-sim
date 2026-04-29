"use client";

import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { BoardClient } from "@/components/board-client";
import { TownhallShell } from "@/components/townhall-shell";
import { getBoardIssues } from "@/lib/issues";
import { BoardIssue } from "@/lib/types";

type Props = {
  repoSlug: string;
};

export function TownhallPageClient({ repoSlug }: Props) {
  const [issues, setIssues] = useState<BoardIssue[]>([]);
  const [loading, setLoading] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function loadIssues() {
      try {
        setLoading(true);
        setErrorMessage(null);
        const nextIssues = await getBoardIssues();
        if (!cancelled) {
          setIssues(nextIssues);
        }
      } catch (error) {
        if (!cancelled) {
          setErrorMessage(error instanceof Error ? error.message : "Unknown GitHub fetch failure");
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    loadIssues();
    return () => {
      cancelled = true;
    };
  }, []);

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
            Only community issues show up here. Anything labeled <code>architecture-sensitive</code> or
            <code> maintainer-only</code> stays off the board.
          </span>
        </div>

        {errorMessage ? (
          <div className="notice danger">
            <span className="notice-icon" aria-hidden>
              x
            </span>
            <span>
              GitHub fetch failed: <code>{errorMessage}</code>
            </span>
          </div>
        ) : null}

        <section id="board">
          <BoardClient issues={issues} repoSlug={repoSlug} loading={loading} />
        </section>
    </TownhallShell>
  );
}
