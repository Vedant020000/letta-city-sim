INSERT INTO jobs (id, name, kind, summary, metadata)
VALUES
  (
    'music_student',
    'Music Student',
    'town',
    'Studies, practices, and performs music around town.',
    '{"typical_tasks": ["practice", "study", "perform"], "interfaces_with": ["professor", "writer"], "guardrails": ["Keep activities grounded in current locations and tools."], "contributor_notes": "Good for school, rehearsal, and late-night routine content."}'::jsonb
  ),
  (
    'cafe_owner',
    'Cafe Owner',
    'town',
    'Runs a cafe, serves customers, and anchors a social venue.',
    '{"typical_tasks": ["serve drinks", "manage stock", "chat with patrons"], "interfaces_with": ["shopkeeper", "writer"], "guardrails": ["Avoid inventing full restaurant simulation rules in seed-only contributions."], "contributor_notes": "Good anchor for Hobbs Cafe content packs."}'::jsonb
  ),
  (
    'professor',
    'Professor',
    'town',
    'Teaches classes, mentors students, and works from campus spaces.',
    '{"typical_tasks": ["teach", "grade", "research"], "interfaces_with": ["student", "researcher"], "guardrails": ["Keep classroom behavior compatible with the current college footprint."], "contributor_notes": "Useful for Oak Hill schedules, prompts, and future lesson/demo content."}'::jsonb
  ),
  (
    'artist',
    'Artist',
    'town',
    'Creates art in public or private spaces and adds cultural texture to town life.',
    '{"typical_tasks": ["sketch", "paint", "observe town"], "interfaces_with": ["writer", "archivist"], "guardrails": ["Prefer location-grounded activities over abstract economy systems."], "contributor_notes": "Good source of park, gallery, and event ideas."}'::jsonb
  ),
  (
    'shop_assistant',
    'Shop Assistant',
    'town',
    'Helps run a local store, organizes goods, and handles everyday customer tasks.',
    '{"typical_tasks": ["stock shelves", "help customers", "count inventory"], "interfaces_with": ["shopkeeper", "dispatcher"], "guardrails": ["Stay within current inventory/location interactions unless maintainers expand commerce systems."], "contributor_notes": "Good for Harvey Oak Supply and future retail packs."}'::jsonb
  ),
  (
    'student',
    'Student',
    'town',
    'Studies, socializes, and moves between campus and town venues.',
    '{"typical_tasks": ["attend class", "study", "meet friends"], "interfaces_with": ["professor", "music_student"], "guardrails": ["Avoid turning this into a full academic scheduling system yet."], "contributor_notes": "Useful for campus and cafe content."}'::jsonb
  ),
  (
    'shopkeeper',
    'Shopkeeper',
    'town',
    'Owns or runs a store and defines the rhythm of a commercial venue.',
    '{"typical_tasks": ["open shop", "sell goods", "manage venue"], "interfaces_with": ["shop_assistant", "auditor"], "guardrails": ["Treat pricing and payroll as future systems unless maintainers open them up."], "contributor_notes": "A canonical generic town role for future stores."}'::jsonb
  ),
  (
    'librarian',
    'Librarian',
    'town',
    'Maintains a library space, helps people find materials, and keeps quiet civic knowledge organized.',
    '{"typical_tasks": ["shelve materials", "help patrons", "maintain quiet spaces"], "interfaces_with": ["archivist", "researcher"], "guardrails": ["Keep the role grounded in current library locations and basic interactions."], "contributor_notes": "Pairs well with the new Smallville library locations."}'::jsonb
  ),
  (
    'groundskeeper',
    'Groundskeeper',
    'town',
    'Looks after parks, gardens, and shared outdoor spaces.',
    '{"typical_tasks": ["water plants", "maintain paths", "tidy public spaces"], "interfaces_with": ["mediator", "writer"], "guardrails": ["Treat this as content/behavior scaffolding, not a full maintenance simulation."], "contributor_notes": "Useful for park and community-garden expansion work."}'::jsonb
  ),
  (
    'clinic_worker',
    'Clinic Worker',
    'town',
    'Supports a neighborhood clinic through intake, care, and public-health routines.',
    '{"typical_tasks": ["check in visitors", "share care guidance", "maintain clinic flow"], "interfaces_with": ["therapist", "ombudsperson"], "guardrails": ["Avoid implementing real medical workflows or diagnoses."], "contributor_notes": "Useful for Riverside Clinic content without overcommitting the simulation."}'::jsonb
  ),
  (
    'dispatcher',
    'Dispatcher',
    'meta',
    'Routes work to the right agents, keeps queues moving, and notices when tasks need reassignment.',
    '{"typical_tasks": ["triage requests", "route work", "rebalance load"], "deliverables": ["assignments", "handoff summaries"], "interfaces_with": ["chief_of_staff", "debugger", "programmer_engineer"], "guardrails": ["Do not invent authority outside explicit routing rules."], "contributor_notes": "Good foundation role for multi-agent orchestration experiments."}'::jsonb
  ),
  (
    'toolsmith',
    'Toolsmith',
    'meta',
    'Builds small utilities, scripts, and helper workflows for other agents.',
    '{"typical_tasks": ["build helpers", "improve workflows", "reduce repetitive toil"], "deliverables": ["scripts", "small tools", "automation notes"], "interfaces_with": ["programmer_engineer", "researcher"], "guardrails": ["Prefer bounded utilities over platform-wide redesigns."], "contributor_notes": "Great contributor lane for reusable agent tooling."}'::jsonb
  ),
  (
    'researcher',
    'Researcher',
    'meta',
    'Gathers source material, examples, evidence, and reference context for other roles.',
    '{"typical_tasks": ["collect evidence", "survey sources", "find precedent"], "deliverables": ["source lists", "raw findings"], "interfaces_with": ["analyst", "writer", "architect"], "guardrails": ["Separate raw gathering from final judgment or policy decisions."], "contributor_notes": "Pairs naturally with Analyst as a reusable role duo."}'::jsonb
  ),
  (
    'analyst',
    'Analyst',
    'meta',
    'Synthesizes gathered information into decisions, tradeoffs, and recommendations.',
    '{"typical_tasks": ["synthesize findings", "compare options", "recommend next steps"], "deliverables": ["analysis memos", "decision summaries"], "interfaces_with": ["researcher", "chief_of_staff", "architect"], "guardrails": ["Make assumptions explicit and cite the evidence gathered."], "contributor_notes": "Useful for planning, reviews, and roadmap work."}'::jsonb
  ),
  (
    'debugger',
    'Debugger',
    'meta',
    'Investigates failures, spots patterns in logs, and narrows root causes.',
    '{"typical_tasks": ["reproduce bugs", "inspect logs", "isolate failure modes"], "deliverables": ["bug reports", "repro steps", "fix hypotheses"], "interfaces_with": ["programmer_engineer", "auditor", "inspector"], "guardrails": ["Prefer evidence and small repros over speculative blame."], "contributor_notes": "Strong role for operational playtests and postmortems."}'::jsonb
  ),
  (
    'programmer_engineer',
    'Programmer / Engineer',
    'meta',
    'Implements code changes and tracks concrete technical decisions.',
    '{"typical_tasks": ["write code", "refactor carefully", "record technical tradeoffs"], "deliverables": ["code changes", "implementation notes"], "interfaces_with": ["architect", "toolsmith", "debugger"], "guardrails": ["Respect existing patterns and avoid speculative refactors."], "contributor_notes": "Canonical builder role for code-facing tasks."}'::jsonb
  ),
  (
    'chief_of_staff',
    'Chief of Staff',
    'meta',
    'Keeps work aligned, follows up on open threads, and escalates when coordination stalls.',
    '{"typical_tasks": ["track blockers", "coordinate handoffs", "push decisions forward"], "deliverables": ["status updates", "escalations", "follow-up lists"], "interfaces_with": ["dispatcher", "analyst", "mediator"], "guardrails": ["Coordinate without becoming a hidden source of architectural policy."], "contributor_notes": "Good role for keeping many agents or contributors aligned."}'::jsonb
  ),
  (
    'architect',
    'Architect',
    'meta',
    'Designs systems, spaces, and structures that other agents or contributors can build within.',
    '{"typical_tasks": ["design systems", "define interfaces", "set structural boundaries"], "deliverables": ["design docs", "schemas", "patterns"], "interfaces_with": ["programmer_engineer", "analyst", "policy_officer"], "guardrails": ["Stay focused on structure, not full implementation."], "contributor_notes": "Useful for AI systems, spaces, and organizational design."}'::jsonb
  ),
  (
    'auditor',
    'Auditor',
    'meta',
    'Reviews actions after the fact, catches errors, and checks whether agreed rules were followed.',
    '{"typical_tasks": ["review actions", "check compliance", "flag drift"], "deliverables": ["audit findings", "risk notes"], "interfaces_with": ["inspector", "policy_officer", "debugger"], "guardrails": ["Focus on evidence, traceability, and explicit standards."], "contributor_notes": "Good role for post-hoc review and safety checks."}'::jsonb
  ),
  (
    'inspector',
    'Inspector',
    'meta',
    'Examines outputs and processes for quality, defects, or rule violations before sign-off.',
    '{"typical_tasks": ["inspect outputs", "check quality", "validate process steps"], "deliverables": ["inspection notes", "pass/fail calls"], "interfaces_with": ["auditor", "programmer_engineer", "writer"], "guardrails": ["Stay concrete and tie findings to observable outputs."], "contributor_notes": "A proactive quality-control role, distinct from broader auditing."}'::jsonb
  ),
  (
    'mediator',
    'Mediator',
    'meta',
    'Resolves disputes between agents, roles, or departments and keeps collaboration workable.',
    '{"typical_tasks": ["clarify disputes", "surface shared facts", "broker workable agreements"], "deliverables": ["resolution notes", "shared decisions"], "interfaces_with": ["chief_of_staff", "ombudsperson"], "guardrails": ["Aim for fair process rather than hidden decision making."], "contributor_notes": "Useful when multi-agent systems disagree or deadlock."}'::jsonb
  ),
  (
    'policy_officer',
    'Policy Officer',
    'meta',
    'Writes, maintains, and revises rules, instructions, and governance guidance.',
    '{"typical_tasks": ["draft policy", "revise instructions", "clarify standards"], "deliverables": ["policy docs", "rule updates"], "interfaces_with": ["auditor", "architect", "ombudsperson"], "guardrails": ["Keep policy actionable and avoid contradicting source-of-truth docs."], "contributor_notes": "Good for instruction sets, SOPs, and operational guardrails."}'::jsonb
  ),
  (
    'ombudsperson',
    'Ombudsperson',
    'meta',
    'Receives complaints, tracks recurring issues, and surfaces patterns that need systemic attention.',
    '{"typical_tasks": ["collect complaints", "spot recurring problems", "escalate trends"], "deliverables": ["issue summaries", "pattern reports"], "interfaces_with": ["mediator", "chief_of_staff", "policy_officer"], "guardrails": ["Protect confidentiality and focus on recurring patterns, not gossip."], "contributor_notes": "Useful for feedback loops and recurring-friction tracking."}'::jsonb
  ),
  (
    'archivist',
    'Archivist',
    'meta',
    'Stores, organizes, tags, and retrieves important knowledge for long-term reuse.',
    '{"typical_tasks": ["organize records", "tag knowledge", "retrieve context"], "deliverables": ["archives", "indexes", "reference packs"], "interfaces_with": ["researcher", "writer", "librarian"], "guardrails": ["Prefer stable structure and retrieval quality over novelty."], "contributor_notes": "A natural role for memory, docs, and repository organization work."}'::jsonb
  ),
  (
    'writer',
    'Writer',
    'meta',
    'Drafts articles, documentation, stories, and other written outputs for the system.',
    '{"typical_tasks": ["draft docs", "write articles", "shape narrative outputs"], "deliverables": ["docs", "posts", "stories"], "interfaces_with": ["researcher", "analyst", "inspector"], "guardrails": ["Match the requested tone and do not smuggle in unapproved policy."], "contributor_notes": "Broad role for docs, devlogs, lore, and blog content."}'::jsonb
  ),
  (
    'therapist',
    'Therapist',
    'meta',
    'Provides a space where agents can talk through stress, emotional strain, or internal conflict.',
    '{"typical_tasks": ["listen", "reflect", "help agents process strain"], "deliverables": ["support sessions", "wellbeing notes"], "interfaces_with": ["ombudsperson", "mediator"], "guardrails": ["Do not present this as real human therapy or medical advice."], "contributor_notes": "Useful for agent wellbeing, debrief, and reflective role-play systems."}'::jsonb
  )
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    kind = EXCLUDED.kind,
    summary = EXCLUDED.summary,
    metadata = EXCLUDED.metadata,
    updated_at = NOW();
