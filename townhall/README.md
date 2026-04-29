# Townhall

Minimal Next.js community board for `letta-city-sim`.

## What it does

- reads open GitHub issues from this repo
- only shows community-facing work
- infers claim status from GitHub issue comments
- encourages one active claim per GitHub username in the UI
- is static-export compatible for GitHub Pages
- serves the public board from a generated local snapshot to avoid GitHub API rate limits on Pages

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

Townhall now generates a local board snapshot during build/deploy so the public site does not depend on live browser-side GitHub API calls.

## Run locally

```powershell
cd townhall
npm install
npm run sync-board
npm run dev
```

`npm run sync-board` refreshes `data/board-snapshot.json` from GitHub. It uses `GITHUB_TOKEN` if available.

## Production / GitHub Pages

Townhall is configured to deploy to the default GitHub Pages URL for this repository:

`https://vedant020000.github.io/letta-city-sim/`

The repository includes a GitHub Actions workflow at:

`/.github/workflows/deploy-townhall-pages.yml`

It builds `townhall/` as a static export and publishes `townhall/out` to GitHub Pages.

The workflow also refreshes the board snapshot before building so claim/comment updates can propagate without exposing a GitHub token in the browser.
