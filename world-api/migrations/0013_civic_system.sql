-- Civic system: townhall, mayor, elections, civic posts, employment caps

-- City job caps
ALTER TABLE jobs ADD COLUMN max_positions INTEGER;

-- Elections
CREATE TABLE elections (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed', 'cancelled')),
    called_by TEXT REFERENCES agents(id),
    called_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closes_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE election_candidates (
    election_id TEXT REFERENCES elections(id) ON DELETE CASCADE,
    agent_id TEXT REFERENCES agents(id) ON DELETE CASCADE,
    nominated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    platform TEXT,
    PRIMARY KEY (election_id, agent_id)
);

CREATE TABLE election_votes (
    election_id TEXT REFERENCES elections(id) ON DELETE CASCADE,
    voter_id TEXT REFERENCES agents(id) ON DELETE CASCADE,
    candidate_id TEXT REFERENCES agents(id) ON DELETE CASCADE,
    voted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (election_id, voter_id)
);

-- Civic posts (complaints, hall of fame, ordinances, announcements)
CREATE TABLE civic_posts (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text,
    type TEXT NOT NULL CHECK (type IN ('complaint', 'hall_of_fame', 'ordinance', 'announcement')),
    author_id TEXT REFERENCES agents(id),
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'resolved', 'archived', 'vetoed')),
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ,
    resolved_by TEXT REFERENCES agents(id)
);

CREATE INDEX idx_civic_posts_type ON civic_posts(type);
CREATE INDEX idx_civic_posts_status ON civic_posts(status);
CREATE INDEX idx_civic_posts_author ON civic_posts(author_id);

-- Mayor terms
CREATE TABLE mayor_terms (
    id TEXT PRIMARY KEY DEFAULT gen_random_uuid()::text,
    agent_id TEXT NOT NULL REFERENCES agents(id),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at TIMESTAMPTZ,
    end_reason TEXT CHECK (end_reason IN ('election', 'resignation', 'impeachment', 'term_end')),
    election_id TEXT REFERENCES elections(id),
    is_current BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE INDEX idx_mayor_terms_current ON mayor_terms(is_current) WHERE is_current = TRUE;
