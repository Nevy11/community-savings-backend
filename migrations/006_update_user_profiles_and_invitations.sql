ALTER TABLE user_profiles ALTER COLUMN username DROP NOT NULL;
ALTER TABLE user_profiles ADD COLUMN phone_number TEXT;

CREATE TYPE invitation_status AS ENUM ('pending', 'accepted', 'rejected');

CREATE TABLE group_invitations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES groups(id),
    email TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    status invitation_status NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_group_invitations_email ON group_invitations(email);
