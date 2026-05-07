-- Add participant status to support join requests and invitations

ALTER TABLE conversation_participants
ADD COLUMN status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'invited', 'requested'));

-- Index for fast lookups of pending requests/invites per agent
CREATE INDEX idx_conv_participant_status ON conversation_participants(agent_id, status)
WHERE status IN ('invited', 'requested');
