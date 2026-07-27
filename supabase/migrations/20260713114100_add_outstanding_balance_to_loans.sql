ALTER TABLE loans ADD COLUMN IF NOT EXISTS outstanding_balance BIGINT NOT NULL DEFAULT 0;
UPDATE loans SET outstanding_balance = principal WHERE outstanding_balance = 0;
