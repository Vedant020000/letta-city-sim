import Link from "next/link";

type Props = {
  repoSlug: string;
  currentPage: "board" | "status";
};

export function TownhallHeader({ repoSlug, currentPage }: Props) {
  return (
    <header className="topbar">
      <div className="brand">
        <span className="brand-mark" aria-hidden />
        <span>
          <div className="brand-name">townhall</div>
          <div className="brand-sub">name not decided city community board</div>
        </span>
      </div>

      <div className="topbar-actions topbar-actions-primary">
        <div className="internal-nav" aria-label="Townhall navigation">
          <Link className={`ghost-link ${currentPage === "board" ? "active" : ""}`} href="/">
            board
          </Link>
          <Link className={`ghost-link ${currentPage === "status" ? "active" : ""}`} href="/status">
            status
          </Link>
        </div>

        <div className="external-nav">
          <a className="ghost-link" href={`https://github.com/${repoSlug}`} target="_blank" rel="noreferrer">
            repo
          </a>
          <a
            className="ghost-link"
            href={`https://github.com/${repoSlug}/issues/new/choose`}
            target="_blank"
            rel="noreferrer"
          >
            propose task
          </a>
        </div>
      </div>
    </header>
  );
}
