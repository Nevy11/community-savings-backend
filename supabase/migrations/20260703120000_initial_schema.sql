-- Community Savings Group (ASCA) initial schema
-- All monetary values are BIGINT (smallest currency unit, e.g. cents/shillings)

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TYPE interest_method AS ENUM ('flat_rate', 'reducing_balance');
CREATE TYPE attendance_status AS ENUM ('present', 'absent', 'late');
CREATE TYPE tx_type AS ENUM (
    'deposit',
    'social_fund_payment',
    'withdrawal',
    'loan_repayment',
    'fine_payment',
    'dividend_payout',
    'loan_disbursement'
);
CREATE TYPE loan_status AS ENUM ('pending', 'approved', 'disbursed', 'repaid', 'defaulted');
CREATE TYPE penalty_type AS ENUM ('attendance', 'loan_late');

CREATE TABLE groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    pool_balance BIGINT NOT NULL DEFAULT 0 CHECK (pool_balance >= 0),
    interest_method interest_method NOT NULL DEFAULT 'flat_rate',
    annual_interest_rate_bps INT NOT NULL DEFAULT 1200 CHECK (annual_interest_rate_bps >= 0),
    absent_fine_amount BIGINT NOT NULL DEFAULT 0 CHECK (absent_fine_amount >= 0),
    late_fine_amount BIGINT NOT NULL DEFAULT 0 CHECK (late_fine_amount >= 0),
    loan_late_penalty_bps INT NOT NULL DEFAULT 500 CHECK (loan_late_penalty_bps >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES groups(id),
    full_name TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_members_group_id ON members(group_id);

CREATE TABLE attendance_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES groups(id),
    member_id UUID NOT NULL REFERENCES members(id),
    meeting_date DATE NOT NULL,
    status attendance_status NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (member_id, meeting_date)
);

CREATE TABLE ledger_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES groups(id),
    member_id UUID NOT NULL REFERENCES members(id),
    amount BIGINT NOT NULL CHECK (amount <> 0),
    tx_type tx_type NOT NULL,
    reference TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ledger_group_id ON ledger_transactions(group_id);
CREATE INDEX idx_ledger_member_id ON ledger_transactions(member_id);
CREATE INDEX idx_ledger_created_at ON ledger_transactions(created_at DESC);

-- Append-only ledger enforcement
CREATE OR REPLACE FUNCTION prevent_ledger_mutation()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'ledger_transactions is append-only';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ledger_no_update
    BEFORE UPDATE ON ledger_transactions
    FOR EACH ROW EXECUTE FUNCTION prevent_ledger_mutation();

CREATE TRIGGER ledger_no_delete
    BEFORE DELETE ON ledger_transactions
    FOR EACH ROW EXECUTE FUNCTION prevent_ledger_mutation();

CREATE TABLE loans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES groups(id),
    member_id UUID NOT NULL REFERENCES members(id),
    principal BIGINT NOT NULL CHECK (principal > 0),
    term_months INT NOT NULL CHECK (term_months > 0),
    status loan_status NOT NULL DEFAULT 'pending',
    approved_at TIMESTAMPTZ,
    disbursed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_loans_group_id ON loans(group_id);
CREATE INDEX idx_loans_member_id ON loans(member_id);

CREATE TABLE loan_guarantors (
    loan_id UUID NOT NULL REFERENCES loans(id) ON DELETE CASCADE,
    member_id UUID NOT NULL REFERENCES members(id),
    PRIMARY KEY (loan_id, member_id)
);

CREATE TABLE penalties (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES groups(id),
    member_id UUID NOT NULL REFERENCES members(id),
    loan_id UUID REFERENCES loans(id),
    penalty_type penalty_type NOT NULL,
    amount BIGINT NOT NULL CHECK (amount > 0),
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    paid BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX idx_penalties_member_id ON penalties(member_id);
CREATE INDEX idx_penalties_loan_id ON penalties(loan_id);
