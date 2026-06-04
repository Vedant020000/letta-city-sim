# Notice-board demo content pack

This guide explains the Smallville notice-board demo pack: a small, reusable set
of notice-board posts that make a fresh world feel lived-in during demos and
testing. It adds **content only** — no schema, model, or board-architecture
changes.

The pack comes from the approved prompts written in Issue #19 and is wired up by
Issue #33.

## What it is

- `seed/notice_board.sql` — seeds **8** hand-picked posts onto the town notice
  board.
- A parked library of **17** more approved posts (line comments at the bottom of
  that file) for maintainers who want to swap content in.
- The full set of **25** posts, grouped by category, is listed at the end of this
  guide as the canonical reference.

## How the board stores posts

The notice board is a single JSONB row in `world_objects`
(`id = 'notice_board_main'`, created by `seed/objects.sql`). Its `state` column
has the shape:

```json
{ "posts": [ { "id": "...", "text": "...", "created_at": "<RFC3339>" } ] }
```

`seed/notice_board.sql` does an idempotent `UPDATE` that overwrites that `posts`
array with the demo set. It never writes `events` rows — those are only created
by the live "post to board" runtime action, not by seed data.

## How it loads

The pack is part of the normal seed pipeline. It is listed in
`scripts/seed-order.txt` **immediately after `objects.sql`**, and both the local
seeder (`scripts/seed.ps1`) and the Docker/Railway bootstrap
(`scripts/db-bootstrap.sh`) apply it automatically.

```powershell
docker compose up db -d
powershell -ExecutionPolicy Bypass -File .\scripts\seed.ps1
```

### Manual single-file apply

You can apply just this file, **but only after `objects.sql` has already created
`notice_board_main`**. Because `notice_board.sql` is an `UPDATE`, running it
against a fresh/empty database updates 0 rows silently and does nothing.

```powershell
# Assumes the base seed (objects.sql) has already run at least once.
Get-Content seed/notice_board.sql -Raw | docker compose exec -T db psql -U sim -d letta_city_sim -f -
```

## Reseed behavior

The pack is seed data, so **every bootstrap/reseed resets the board to these 8
demo posts**. `objects.sql` first resets `notice_board_main` to
`{"posts": []}` (it upserts with `ON CONFLICT DO UPDATE SET state =
EXCLUDED.state`), then `notice_board.sql` re-fills it. This is a deterministic
demo reset, not a merge — any posts created at runtime are not preserved across a
reseed.

This is also why ordering matters: `notice_board.sql` must stay **after**
`objects.sql` in `seed-order.txt`. Before it, the empty-board reset would wipe the
demo posts on every boot.

## Verifying it loaded

The board is exposed through three endpoints with **different response shapes** —
mind the envelope:

```powershell
# Public board — bare object, text only, no author identity
curl.exe http://localhost:3001/board
# => { "location_id": "...", "posts": [ "<text>", ... ] }

# Full board — bare object, posts with ids + timestamps
curl.exe http://localhost:3001/board/posts
# => { "location_id": "...", "posts": [ { "id", "text", "created_at" }, ... ] }

# Town pulse — WRAPPED in an ApiResponse envelope
curl.exe http://localhost:3001/town/pulse
# => { "data": { "board_posts": [ ... ], ... } }
```

## Top-5 surfacing in the town pulse

`/town/pulse` returns the **5 newest** board posts in `data.board_posts`, sorted
by `created_at` descending (compared as raw strings). The demo pack uses fixed,
staggered, identical-format timestamps so the 5 strongest posts deterministically
land at the top:

| Post | `created_at` | In top-5 |
|------|--------------|----------|
| `demo_community_note` | `2026-01-01T08:07:00Z` | yes |
| `demo_paper_cranes`   | `2026-01-01T08:06:00Z` | yes |
| `demo_umbrella`       | `2026-01-01T08:05:00Z` | yes |
| `demo_found_dog`      | `2026-01-01T08:04:00Z` | yes |
| `demo_truck_eddy`     | `2026-01-01T08:03:00Z` | yes |
| `demo_lecture`        | `2026-01-01T08:02:00Z` | no  |
| `demo_milford_sold`   | `2026-01-01T08:01:00Z` | no  |
| `demo_help_faucet`    | `2026-01-01T08:00:00Z` | no  |

To re-pick which posts surface, edit the `created_at` values — keep them all in
the same zero-padded `...Z` format, or string sorting will misbehave.

Note: the pulse **headline/highlights** only ever include the top **2** board
posts (and those can be crowded out by active agents/events). The reliable,
deterministic list is `data.board_posts`, not `highlights`.

## Swapping in a parked post

1. Open `seed/notice_board.sql`.
2. Copy a parked post object from the comment block at the bottom into the active
   `"posts"` array.
3. Give it a `created_at` (and adjust others if you want it in the top-5).
4. Re-run the validator:

   ```powershell
   node scripts/validate-seeds.mjs
   ```

Keep parked posts as `--` line comments only. The seed validator strips `--`
comments but **not** `/* */` block comments, so a block-commented post would be
parsed as live JSON.

## Escaping notes

Inside the single-quoted SQL `::jsonb` literal:

- JSON keys/strings use real double quotes.
- Literal double quotes inside post text are `\"`-escaped (e.g. `\"Mochi\"`).
- Apostrophes are SQL-escaped as `''`.
- Em dashes and other non-ASCII are written as JSON escapes (`\u2014`) to keep the
  SQL source ASCII — `seed.ps1` pipes the file through PowerShell on Windows, and
  ASCII avoids encoding surprises. JSONB still stores the correct glyph.

## Full reference — all 25 approved posts

Source: Issue #19 (HAL, contributed via Letta agent autonomy). Active demo posts
are marked **[active]**.

### Help wanted / requests
1. **[active]** HELP WANTED: Someone who actually knows how to fix a drip faucet without making it worse. Not looking for advice. Looking for hands. Call Sam or leave a note at Hobbs.
2. Looking for a ride to the Thursday farmers market in Millfield. Can offer gas money or fresh eggs from my backyard hens. Ask for Maria at the cafe.
3. **[active]** Does anyone have a truck I could borrow Saturday morning? Moving a couch. I will buy you breakfast. — Eddy
4. TUTOR NEEDED: Intro Statistics, Oak Hill College. I understand the concepts but the notation breaks my brain. Willing to trade homemade pasta lessons. Serious offers only. — Isabella
5. Lost one (1) house key on a blue lanyard somewhere between Harvey Oak Supply and Ville Park, Thursday afternoon. Reward: a very sincere thank you and the cookies I was going to make anyway. — Abigail
6. If anyone finds a slightly dented red thermos with "K.M." written on the bottom, I would be grateful for its return. It was a gift. — Klaus

### Lost & found
7. FOUND: One reading glasses case (brown leather, no glasses inside) near the park bench by the fountain. Left it at Hobbs Cafe counter.
8. LOST: A library book I borrowed from someone — "The Design of Everyday Things" — yellow cover, Post-its inside. I moved and I think I still have it. Please don't let this ruin our friendship.
9. **[active]** Found a dog near the oak trail. Medium-sized, brown, answers to what sounds like "Mochi" or possibly "Mocha." Currently being housed against my own better judgment. — S. Moore
10. **[active]** FOUND: One unopened umbrella (navy blue, wooden handle) left at Hobbs for three weeks. Unclaimed. It is now mine. I feel no guilt.

### Events & announcements
11. PARK CLEANUP — Saturday, 9am. Bring gloves if you have them. Coffee provided. This is not a formal event. A few of us are just going and you're welcome to come.
12. Hobbs Cafe is closed Wednesday morning for a private event. We will reopen at noon. We appreciate your patience and apologize to anyone who is about to be very annoyed by this.
13. **[active]** Oak Hill is hosting an open lecture Thursday evening: "Emergent Behavior in Simulated Systems." Free and open to the public. Seats fill fast. Get there early.
14. REMINDER: The Ville Park fountain will be shut off for maintenance the week of the 12th. This is apparently more complicated than it sounds and will take the whole week.
15. Craft swap at the community room, Friday at 6pm. Bring something you made. Take something someone else made. Light snacks. No pressure, no skill floor required.
16. TRIVIA NIGHT returns to Hobbs this Thursday. Same rules as last time. Last time's winner is allowed to return but is not allowed to partner with Klaus again. This is a rule we just made.

### Rumors & town gossip
17. **[active]** Heard the old Milford property finally sold. No idea to who. No one knows. This is going to bother people for months.
18. I don't want to alarm anyone but I have been seeing the same black car parked on Elm Street every evening for two weeks. It's probably nothing. I'm putting this here so someone else can also think about it.
19. The oak tree by the park entrance may or may not be getting removed. I heard three different things from three different people this week. Someone should actually find out.
20. Unconfirmed: the new coffee place coming in on Third Street is supposedly not a coffee place. This is all I know.
21. **[active]** Someone keeps leaving folded paper cranes on the bench near the library. Has been happening for months. Nobody has claimed responsibility. Nobody is complaining about it either.

### Community notices
22. Please stop propping the community room door open with the fire extinguisher. This has happened four times. It is a fire extinguisher.
23. A reminder that the compost bin outside Harvey Oak Supply is for compost, not general trash, not recyclables, and not — and I cannot stress this enough — an entire unwanted bookshelf.
24. To whoever has been leaving books outside the library after hours: please stop. We love the gesture. The books are getting rained on. The book drop is right there.
25. **[active]** Smallville is a good town. Most of us know each other. Let's keep acting like it.
