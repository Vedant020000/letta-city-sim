import Link from "next/link";
import { TownhallShell } from "@/components/townhall-shell";
import { getRepoSlug } from "@/lib/github";
import { builtSoFar, contributionSplit, projectStatus, roadmapColumns, statusSnapshot } from "@/lib/status";

export default function StatusPage() {
  const repoSlug = getRepoSlug();

  return (
    <TownhallShell repoSlug={repoSlug} currentPage="status">
      <section className="hero status-hero">
        <div className="hero-eyebrow">city sim status</div>

        <h1 className="hero-title">
          where <span className="gradient">name not decided city</span> stands right now
        </h1>

        <p className="hero-tagline">
          <strong>{projectStatus.overallStatus}</strong> {projectStatus.summary}
        </p>

        <div className="cta-row">
          <Link className="btn btn-primary" href="/">
            open task board
          </Link>
          <a
            className="btn btn-ghost"
            href={`https://github.com/${repoSlug}/blob/main/CONTRIBUTING.md`}
            target="_blank"
            rel="noreferrer"
          >
            contributing guide
          </a>
        </div>

        <div className="pill-row">
          <span className="pill accent">world foundation live</span>
          <span className="pill">frontend engine exists</span>
          <span className="pill">townhall public</span>
          <span className="pill">next: first agent proving</span>
        </div>
      </section>

      <section className="status-grid">
        {statusSnapshot.map((item) => (
          <article className="status-card" key={item.title}>
            <div className="status-card-label">{item.title}</div>
            <h2 className="status-card-value">{item.value}</h2>
            <p className="status-card-copy">{item.description}</p>
          </article>
        ))}
      </section>

      <section className="status-section paper-section">
        <div className="section-head">
          <h2>built so far</h2>
          <span className="count">actual shipped pieces</span>
        </div>
        <ul className="status-list">
          {builtSoFar.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </section>

      <section className="status-section paper-section">
        <div className="section-head">
          <h2>roadmap snapshot</h2>
          <span className="count">now / next / later</span>
        </div>
        <div className="roadmap-grid">
          {roadmapColumns.map((column) => (
            <article className="roadmap-card" key={column.title}>
              <h3>{column.title}</h3>
              <ul className="status-list compact">
                {column.items.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            </article>
          ))}
        </div>
      </section>

      <section className="status-section paper-section">
        <div className="section-head">
          <h2>who is working on what</h2>
          <span className="count">maintainers vs community</span>
        </div>
        <div className="split-grid">
          {contributionSplit.map((column) => (
            <article className="roadmap-card" key={column.title}>
              <h3>{column.title}</h3>
              <ul className="status-list compact">
                {column.items.map((item) => (
                  <li key={item}>{item}</li>
                ))}
              </ul>
            </article>
          ))}
        </div>
      </section>

      <section className="status-section paper-section devlog-placeholder">
        <div className="section-head">
          <h2>dev log / updates</h2>
          <span className="count">coming next</span>
        </div>
        <p className="status-body-copy">
          This is where the public thread of city-sim updates will live. For now, the status page is the clean snapshot;
          later we can add timestamped dev logs, screenshots, and short progress posts below it.
        </p>
      </section>
    </TownhallShell>
  );
}
