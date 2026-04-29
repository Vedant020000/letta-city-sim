import type { ReactNode } from "react";
import { TownhallFooter } from "@/components/townhall-footer";
import { TownhallHeader } from "@/components/townhall-header";

type Props = {
  repoSlug: string;
  currentPage: "board" | "status";
  children: ReactNode;
};

export function TownhallShell({ repoSlug, currentPage, children }: Props) {
  return (
    <main className="page">
      <div className="pixel-cloud pixel-cloud-left" aria-hidden />
      <div className="pixel-cloud pixel-cloud-right" aria-hidden />

      <div className="shell">
        <TownhallHeader repoSlug={repoSlug} currentPage={currentPage} />
        {children}
        <TownhallFooter repoSlug={repoSlug} />
      </div>
    </main>
  );
}
