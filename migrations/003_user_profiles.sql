CREATE TYPE user_theme AS ENUM ('light', 'dark');
CREATE TYPE user_role AS ENUM ('administrator', 'member');

CREATE TABLE user_profiles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    auth_user_id UUID UNIQUE NOT NULL,
    username TEXT UNIQUE NOT NULL,
    full_name TEXT,
    preferred_theme user_theme NOT NULL DEFAULT 'light',
    role user_role NOT NULL DEFAULT 'administrator',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_user_profiles_username ON user_profiles(username);
CREATE INDEX idx_user_profiles_auth_user_id ON user_profiles(auth_user_id);
