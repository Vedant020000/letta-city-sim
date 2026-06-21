-- notice_board.sql - Smallville notice-board demo content pack (Issue #33)
--
-- Turns the approved notice-board prompts from Issue #19 into a concrete,
-- reusable demo pack. Seeds 8 hand-picked posts onto the town notice board so a
-- fresh world feels lived-in during demos and testing.
--
-- HOW IT WORKS
--   The notice board is a single JSONB row in world_objects
--   (id = 'notice_board_main', created by seed/objects.sql). Its state has the
--   shape {"posts": [ { "id", "text", "created_at" } ]}. This file does an
--   idempotent UPDATE that overwrites that posts array with the demo set.
--
-- ORDERING (load-bearing)
--   This file MUST run AFTER objects.sql in scripts/seed-order.txt. objects.sql
--   upserts notice_board_main and RESETS its state to {"posts": []} on every
--   reseed; this file then re-fills it. Running before objects.sql would leave
--   the board empty after every bootstrap.
--
-- IDEMPOTENT
--   Re-applying the full seed sequence always yields exactly these 8 posts.
--   It overwrites, never appends -- safe to run on every boot.
--
-- TIMESTAMPS / TOWN PULSE
--   created_at values are fixed and staggered (RFC3339, identical zero-padded
--   ...Z format). /town/pulse shows the 5 newest posts (sorted by created_at,
--   lexicographically descending). The 5 strongest posts are given the newest
--   timestamps so they deterministically surface in the pulse:
--     demo_community_note (08:07) > demo_paper_cranes (08:06) >
--     demo_umbrella (08:05) > demo_found_dog (08:04) > demo_truck_eddy (08:03)
--   The remaining 3 (lecture 08:02, milford 08:01, faucet 08:00) are older.
--
-- ESCAPING
--   Inside the single-quoted SQL ::jsonb literal: JSON uses real double quotes
--   for keys/strings; literal double quotes within post text are \"-escaped;
--   apostrophes are SQL-escaped as ''; em dashes are JSON \u2014 escapes to keep
--   this source ASCII (seed.ps1 pipes through PowerShell on Windows).
--
-- A library of 17 additional approved posts is parked at the bottom of this file
-- (line comments) for maintainers who want to swap content in. The full set of
-- 25 lives in docs/guides/notice-board-demo-pack.md.

UPDATE world_objects
SET state = '{
  "posts": [
    { "id": "demo_community_note", "text": "Smallville is a good town. Most of us know each other. Let''s keep acting like it.", "created_at": "2026-01-01T08:07:00Z" },
    { "id": "demo_paper_cranes", "text": "Someone keeps leaving folded paper cranes on the bench near the library. Has been happening for months. Nobody has claimed responsibility. Nobody is complaining about it either.", "created_at": "2026-01-01T08:06:00Z" },
    { "id": "demo_umbrella", "text": "FOUND: One unopened umbrella (navy blue, wooden handle) left at Hobbs for three weeks. Unclaimed. It is now mine. I feel no guilt.", "created_at": "2026-01-01T08:05:00Z" },
    { "id": "demo_found_dog", "text": "Found a dog near the oak trail. Medium-sized, brown, answers to what sounds like \"Mochi\" or possibly \"Mocha.\" Currently being housed against my own better judgment. \u2014 S. Moore", "created_at": "2026-01-01T08:04:00Z" },
    { "id": "demo_truck_eddy", "text": "Does anyone have a truck I could borrow Saturday morning? Moving a couch. I will buy you breakfast. \u2014 Eddy", "created_at": "2026-01-01T08:03:00Z" },
    { "id": "demo_lecture", "text": "Oak Hill is hosting an open lecture Thursday evening: \"Emergent Behavior in Simulated Systems.\" Free and open to the public. Seats fill fast. Get there early.", "created_at": "2026-01-01T08:02:00Z" },
    { "id": "demo_milford_sold", "text": "Heard the old Milford property finally sold. No idea to who. No one knows. This is going to bother people for months.", "created_at": "2026-01-01T08:01:00Z" },
    { "id": "demo_help_faucet", "text": "HELP WANTED: Someone who actually knows how to fix a drip faucet without making it worse. Not looking for advice. Looking for hands. Call Sam or leave a note at Hobbs.", "created_at": "2026-01-01T08:00:00Z" }
  ]
}'::jsonb
WHERE id = 'notice_board_main';

-- -----------------------------------------------------------------------------
-- PARKED POSTS - the remaining 17 approved #19 prompts, grouped by category.
-- To use one: copy its object into the "posts" array above (give it a stable
-- id + a created_at), then re-run `node scripts/validate-seeds.mjs`.
-- These are line comments only (never /* */) so the seed validator ignores them.
-- -----------------------------------------------------------------------------
--
-- HELP WANTED / REQUESTS
--   { "id": "demo_ride_market",  "text": "Looking for a ride to the Thursday farmers market in Millfield. Can offer gas money or fresh eggs from my backyard hens. Ask for Maria at the cafe." }
--   { "id": "demo_tutor_stats",  "text": "TUTOR NEEDED: Intro Statistics, Oak Hill College. I understand the concepts but the notation breaks my brain. Willing to trade homemade pasta lessons. Serious offers only. \u2014 Isabella" }
--   { "id": "demo_lost_key",     "text": "Lost one (1) house key on a blue lanyard somewhere between Harvey Oak Supply and Ville Park, Thursday afternoon. Reward: a very sincere thank you and the cookies I was going to make anyway. \u2014 Abigail" }
--   { "id": "demo_lost_thermos", "text": "If anyone finds a slightly dented red thermos with \"K.M.\" written on the bottom, I would be grateful for its return. It was a gift. \u2014 Klaus" }
--
-- LOST & FOUND
--   { "id": "demo_found_glasses", "text": "FOUND: One reading glasses case (brown leather, no glasses inside) near the park bench by the fountain. Left it at Hobbs Cafe counter." }
--   { "id": "demo_lost_book",     "text": "LOST: A library book I borrowed from someone \u2014 \"The Design of Everyday Things\" \u2014 yellow cover, Post-its inside. I moved and I think I still have it. Please don''t let this ruin our friendship." }
--
-- EVENTS & ANNOUNCEMENTS
--   { "id": "demo_park_cleanup",  "text": "PARK CLEANUP \u2014 Saturday, 9am. Bring gloves if you have them. Coffee provided. This is not a formal event. A few of us are just going and you''re welcome to come." }
--   { "id": "demo_hobbs_closed",  "text": "Hobbs Cafe is closed Wednesday morning for a private event. We will reopen at noon. We appreciate your patience and apologize to anyone who is about to be very annoyed by this." }
--   { "id": "demo_fountain_maint","text": "REMINDER: The Ville Park fountain will be shut off for maintenance the week of the 12th. This is apparently more complicated than it sounds and will take the whole week." }
--   { "id": "demo_craft_swap",    "text": "Craft swap at the community room, Friday at 6pm. Bring something you made. Take something someone else made. Light snacks. No pressure, no skill floor required." }
--   { "id": "demo_trivia_night",  "text": "TRIVIA NIGHT returns to Hobbs this Thursday. Same rules as last time. Last time''s winner is allowed to return but is not allowed to partner with Klaus again. This is a rule we just made." }
--
-- RUMORS & TOWN GOSSIP
--   { "id": "demo_black_car",     "text": "I don''t want to alarm anyone but I have been seeing the same black car parked on Elm Street every evening for two weeks. It''s probably nothing. I''m putting this here so someone else can also think about it." }
--   { "id": "demo_oak_tree",      "text": "The oak tree by the park entrance may or may not be getting removed. I heard three different things from three different people this week. Someone should actually find out." }
--   { "id": "demo_third_street",  "text": "Unconfirmed: the new coffee place coming in on Third Street is supposedly not a coffee place. This is all I know." }
--
-- COMMUNITY NOTICES
--   { "id": "demo_fire_extinguisher", "text": "Please stop propping the community room door open with the fire extinguisher. This has happened four times. It is a fire extinguisher." }
--   { "id": "demo_compost_bin",       "text": "A reminder that the compost bin outside Harvey Oak Supply is for compost, not general trash, not recyclables, and not \u2014 and I cannot stress this enough \u2014 an entire unwanted bookshelf." }
--   { "id": "demo_library_books",     "text": "To whoever has been leaving books outside the library after hours: please stop. We love the gesture. The books are getting rained on. The book drop is right there." }
