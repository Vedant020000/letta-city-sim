-- Add 'left' status to conversation_participants for clean departure tracking

ALTER TABLE conversation_participants
DROP CONSTRAINT conversation_participants_status_check;

ALTER TABLE conversation_participants
ADD CONSTRAINT conversation_participants_status_check
    CHECK (status IN ('active', 'invited', 'requested', 'left'));
