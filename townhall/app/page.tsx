import { TownhallPageClient } from "@/components/townhall-page-client";
import { getRepoSlug } from "@/lib/github";

export default async function HomePage() {
  const repoSlug = getRepoSlug();
  return <TownhallPageClient repoSlug={repoSlug} />;
}
