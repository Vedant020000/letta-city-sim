# Deploying to Railway

This guide is the simplest supported path for creating a public Railway deployment of `letta-city-sim` from this repo.

Use this when you want a publicly viewable city-sim instance that:

- serves the frontend and World API from one app URL
- bootstraps the database automatically
- redeploys itself when new commits land on GitHub `main`

This repo is already prepared for that flow:

- `railway.toml` points Railway at `Dockerfile.bundle`
- `Dockerfile.bundle` builds the bundled frontend + world-api image
- `/app/scripts/db-bootstrap.sh` runs before deploy to apply migrations and idempotent seed SQL

## What Railway should host

You need two things:

1. **App service** - deployed from this GitHub repo
2. **Postgres service** - Railway Postgres, used by the app via `DATABASE_URL`

The app service is the public thing everyone sees.
The Postgres service stays internal.

## One-time setup

### 1. Create a Railway project

In Railway:

1. Create a new project
2. Choose **Deploy from GitHub repo**
3. Select this repository: `Vedant020000/letta-city-sim`

Because `railway.toml` is already in the repo, Railway should use the bundled Docker deploy path automatically.

### 2. Add Postgres

Add a **Postgres** service to the same Railway project.

You do not need to expose Postgres publicly.

### 3. Configure the app service variables

Set these on the app service.

#### Required

| Variable | Value |
| --- | --- |
| `DATABASE_URL` | `${{Postgres.DATABASE_URL}}` |
| `SIM_API_KEY` | generate a strong random secret |

#### Optional but useful

| Variable | Suggested value | Why |
| --- | --- | --- |
| `DB_WAIT_ATTEMPTS` | `60` | Gives Postgres time to come up during deploy |
| `DB_WAIT_SECONDS` | `2` | Retry delay for DB bootstrap/startup |

Do **not** override `PORT` unless you know exactly why. Railway will provide it.

### 4. Deploy

Deploy the app service.

Railway should:

1. build from `Dockerfile.bundle`
2. run `/app/scripts/db-bootstrap.sh` as the pre-deploy step
3. start the bundled app
4. use `/api/health` as the healthcheck

### 5. Generate a public domain

Once the deploy is healthy:

1. open the app service in Railway
2. go to **Settings** -> **Networking**
3. generate a public domain

That public domain becomes the public city URL.

## What auto-redeploy means here

If the Railway app service is connected to this GitHub repo/branch, Railway will automatically rebuild and redeploy when new commits land on that tracked branch.

For the public status deployment, the intended branch is:

- **`main`**

So if Vedant merges a new city-sim change to `main`, Railway should redeploy the public instance automatically.

## Smoke checks after deploy

Replace `YOUR_URL` below with the generated Railway URL.

```powershell
$PUBLIC_URL = "https://YOUR_URL"

curl.exe "$PUBLIC_URL/api/health"
curl.exe "$PUBLIC_URL/api/world/time"
curl.exe "$PUBLIC_URL/api/agents"
curl.exe "$PUBLIC_URL/api/jobs"
curl.exe "$PUBLIC_URL/api/town/pulse"
```

Then open the frontend:

```powershell
start "$PUBLIC_URL"
```

## What this public deploy is for

This is meant to be a **public spectator/status instance**:

- people can open the frontend and see the current city state
- people can query public read endpoints
- maintainers can use auth to perform controlled mutations

If you need a maintainers-only environment, create a separate Railway project instead of overloading the public one.

## Optional next step: true one-click template link

This repo is now prepared for a simple Railway deploy, but a real **Deploy on Railway** button with a Railway template URL usually requires a Railway-side template publishing step.

So the practical split is:

- **repo work**: done here
- **template URL generation**: done in Railway when/if you publish this as a reusable template

Once that URL exists, add it to the README as the public one-click entry point.
