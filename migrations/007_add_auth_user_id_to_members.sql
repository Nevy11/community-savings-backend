ALTER TABLE members ADD COLUMN auth_user_id UUID REFERENCES user_profiles(auth_user_id) ON DELETE SET NULL;
