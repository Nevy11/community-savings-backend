ALTER TABLE loans ADD COLUMN outstanding_balance BIGINT NOT NULL DEFAULT 0;
UPDATE loans SET outstanding_balance = principal;
