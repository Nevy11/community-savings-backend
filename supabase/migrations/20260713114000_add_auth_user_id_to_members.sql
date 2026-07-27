ALTER TABLE members ADD COLUMN IF NOT EXISTS auth_user_id UUID REFERENCES user_profiles(auth_user_id) ON DELETE SET NULL;
