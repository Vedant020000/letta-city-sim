# Townhall

Minimal Next.js community board for `letta-city-sim`.

## What it does

- reads open GitHub issues from this repo
- only shows community-facing work
- infers claim status from GitHub issue comments
- encourages one active claim per GitHub username in the UI
- is static-export compatible for GitHub Pages

## Claim flow

Claims are intentionally lightweight and happen on GitHub:

- claim an issue by commenting `/claim`
- release it by commenting `/release`

The board reads comments and derives claim state from that.

## Environment

Optional repo override:

```powershell
$env:NEXT_PUBLIC_GITHUB_REPO_OWNER="Vedant020000"
$env:NEXT_PUBLIC_GITHUB_REPO_NAME="letta-city-sim"
```

The live board fetches GitHub issue data client-side from the browser so it can run on GitHub Pages without a server.

## Run locally

```powershell
cd townhall
npm install
npm run dev
```

## Production / GitHub Pages

Townhall is configured to deploy to the default GitHub Pages URL for this repository:

`https://vedant020000.github.io/letta-city-sim/`

The repository includes a GitHub Actions workflow at:

`/.github/workflows/deploy-townhall-pages.yml`

It builds `townhall/` as a static export and publishes `townhall/out` to GitHub Pages.
