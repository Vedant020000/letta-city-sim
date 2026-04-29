import type { NextConfig } from "next";

const repoName = process.env.NEXT_PUBLIC_GITHUB_REPO_NAME || process.env.GITHUB_REPO_NAME || "letta-city-sim";
const isProd = process.env.NODE_ENV === "production";

const nextConfig: NextConfig = {
  output: "export",
  trailingSlash: true,
  images: {
    unoptimized: true,
  },
  basePath: isProd ? `/${repoName}` : "",
  assetPrefix: isProd ? `/${repoName}/` : undefined,
};

export default nextConfig;
