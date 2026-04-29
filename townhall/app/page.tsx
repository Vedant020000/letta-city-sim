import boardSnapshot from "@/data/board-snapshot.json";
import { TownhallPageClient } from "@/components/townhall-page-client";
import { getRepoSlug } from "@/lib/github";
import type { BoardSnapshot } from "@/lib/types";

export default async function HomePage() {
  const repoSlug = getRepoSlug();
  const snapshot = boardSnapshot as BoardSnapshot;

  return (
    <TownhallPageClient
      repoSlug={repoSlug}
      initialIssues={snapshot.issues}
      snapshotGeneratedAt={snapshot.generated_at}
    />
  );
}
