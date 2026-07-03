# Community Savings Backend

Production-grade REST API for a **Community Savings Group Management System** built on the **ASCA** (Accumulating Savings and Credit Association) model. The API powers member onboarding, contributions, loans, penalties, dividends, and M-Pesa payment reconciliation.

**Live API:** [https://community-savings-backend.onrender.com](https://community-savings-backend.onrender.com)

**Database:** [Supabase](https://yzbpzkhxtdlwhiyjehel.supabase.co) (PostgreSQL)

---

## Table of Contents

- [Features](#features)
- [Tech Stack](#tech-stack)
- [Architecture](#architecture)
- [Project Structure](#project-structure)
- [Prerequisites](#prerequisites)
- [Environment Variables](#environment-variables)
- [Getting Started](#getting-started)
- [Database & Migrations](#database--migrations)
- [API Reference](#api-reference)
- [Financial & Concurrency Rules](#financial--concurrency-rules)
- [M-Pesa Integration](#m-pesa-integration)
- [Deployment (Render)](#deployment-render)
- [Testing](#testing)
- [Security Notes](#security-notes)

---

## Features

| Module | Description |
|--------|-------------|
| **Members & Attendance** | Onboard members, track active status, log meeting attendance. Absent/late status auto-creates attendance fines. |
| **Transaction Ledger** | Append-only ledger for deposits, social fund payments, withdrawals, repayments, fines, and dividends. |
| **Loans** | Request loans, assign guarantors, approve/disburse with pool balance checks and row-level locking. |
| **Penalties** | Calculate and apply stacked or fixed-rate penalties for late loan repayments. |
| **Finance Engine** | Flat-rate and reducing-balance amortization; time-weighted dividend distribution. |
| **M-Pesa Gateway** | Webhook callback with HMAC signature verification and automatic ledger entries. |

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Language | Rust (edition 2024) |
| Web framework | [Axum](https://github.com/tokio-rs/axum) 0.8 |
| Database | PostgreSQL via [SQLx](https://github.com/launchbadge/sqlx) 0.8 |
| Hosting | [Render](https://render.com) |
| Database host | [Supabase](https://supabase.com) |
| Serialization | Serde / JSON |

---

## Architecture

The codebase follows a **modular, layered architecture** — no monolithic `main.rs`.

```
HTTP Request
     │
     ▼
┌─────────────┐
│  handlers/  │  Route handlers, extractors, HTTP responses
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  services/  │  Business logic (finance math, validation, M-Pesa)
└──────┬──────┘
       │
       ▼
┌─────────────┐
│   models/   │  Database schemas & request/response types
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  PostgreSQL │  Supabase (append-only ledger, row locks)
└─────────────┘
```

**Cross-cutting concerns:**
- `config.rs` — environment variables and connection pool
- `error.rs` — centralized `AppError` enum mapped to HTTP status codes

---

## Project Structure

```
community-savings-backend/
├── Cargo.toml
├── README.md
├── .env                          # Local secrets (git-ignored)
├── migrations/
│   └── 001_initial_schema.sql    # Source migration
├── supabase/
│   ├── config.toml
│   └── migrations/
│       └── 20260703120000_initial_schema.sql
└── src/
    ├── main.rs                   # Entry point & router wiring
    ├── config.rs                 # Env vars & DB pool
    ├── error.rs                  # AppError → HTTP responses
    ├── handlers/
    │   ├── mod.rs
    │   ├── members.rs            # Members & attendance
    │   ├── transactions.rs       # Append-only ledger
    │   ├── loans.rs              # Loan lifecycle
    │   ├── penalties.rs          # Fine calculation & application
    │   └── mpesa.rs              # M-Pesa webhook
    ├── models/
    │   ├── mod.rs
    │   ├── member.rs
    │   ├── transaction.rs
    │   ├── loan.rs
    │   ├── group.rs
    │   └── penalty.rs
    └── services/
        ├── mod.rs
        ├── finance.rs            # Amortization & dividend math
        ├── validation.rs         # Input validation
        └── mpesa.rs              # Signature verification
```

---

## Prerequisites

- **Rust** 1.85+ ([rustup](https://rustup.rs))
- **Supabase CLI** (optional, for migrations): `npm install -g supabase`
- **PostgreSQL client** (optional): for manual SQL queries

---

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `DATABASE_URL` | Yes | PostgreSQL connection string (Supabase pooler, session mode, port `5432`) |
| `PORT` | No | HTTP port (default: `3000`). Render sets this automatically. |
| `MPESA_CALLBACK_SECRET` | No | HMAC secret for M-Pesa webhook verification (default: dev placeholder) |

### Example `.env` (local development)

```env
DATABASE_URL="postgresql://postgres.<project-ref>:<url-encoded-password>@aws-0-eu-west-3.pooler.supabase.com:5432/postgres"
MPESA_CALLBACK_SECRET="your-production-secret"
```

> **Important:** If your database password contains special characters (`@`, `#`, `%`, etc.), they must be [URL-encoded](https://developer.mozilla.org/en-US/docs/Glossary/Percent-encoding) in `DATABASE_URL`. For example, `@` becomes `%40`.

Get your connection details from **Supabase Dashboard → Project Settings → Database**.

---

## Getting Started

### 1. Clone and install dependencies

```bash
git clone <repository-url>
cd community-savings-backend
```

### 2. Configure environment

Create a `.env` file in the project root (see [Environment Variables](#environment-variables)).

### 3. Run the server

```bash
cargo run
```

The server binds to `0.0.0.0` on the configured port.

### 4. Verify

```bash
curl http://localhost:3000/ping
# pong

curl http://localhost:3000/health
# {"status":"ok"}
```

---

## Database & Migrations

### Schema overview

| Table | Purpose |
|-------|---------|
| `groups` | Savings group settings, pool balance, interest rates, fine amounts |
| `members` | Group members |
| `attendance_records` | Meeting attendance (present / absent / late) |
| `ledger_transactions` | **Append-only** financial ledger |
| `loans` | Loan requests and lifecycle |
| `loan_guarantors` | Guarantor assignments per loan |
| `penalties` | Assessed fines (attendance & loan late fees) |

### Apply migrations (Supabase CLI)

```bash
# Login (one-time)
supabase login

# Link to your project
supabase link --project-ref yzbpzkhxtdlwhiyjehel

# Push migrations to remote database
supabase db push
```

### Verify tables

```bash
supabase db query --linked "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' ORDER BY table_name;"
```

---

## API Reference

Base URL: `https://community-savings-backend.onrender.com`

All `/api/*` endpoints return JSON. Errors follow:

```json
{ "error": "description of the problem" }
```

### Health

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/ping` | Simple liveness check → `pong` |
| `GET` | `/health` | JSON health status |

---

### Members — `/api/members`

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/members` | List all members |
| `POST` | `/api/members` | Onboard a new member |
| `GET` | `/api/members/{id}` | Get member by ID |
| `PATCH` | `/api/members/{id}` | Update member details / active status |
| `GET` | `/api/members/{id}/attendance` | List attendance records (`?group_id=`) |
| `POST` | `/api/members/{id}/attendance` | Record meeting attendance |

**Create member:**

```json
POST /api/members
{
  "group_id": "uuid",
  "full_name": "Jane Doe",
  "phone_number": "254712345678"
}
```

**Record attendance** (auto-fines on `absent` or `late`):

```json
POST /api/members/{id}/attendance
{
  "group_id": "uuid",
  "meeting_date": "2026-07-03",
  "status": "present"
}
```

`status` values: `present`, `absent`, `late`

---

### Transactions — `/api/transactions`

Append-only ledger. **No update or delete endpoints.**

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/transactions` | List ledger entries (`?group_id=`, `?member_id=`) |
| `POST` | `/api/transactions` | Append a new ledger entry |
| `GET` | `/api/transactions/{id}` | Get entry by ID |

**Append entry:**

```json
POST /api/transactions
{
  "group_id": "uuid",
  "member_id": "uuid",
  "amount": 50000,
  "tx_type": "deposit",
  "reference": "weekly contribution"
}
```

**Transaction types (`tx_type`):**

| Type | Amount sign | Description |
|------|-------------|-------------|
| `deposit` | Positive | Member contribution |
| `social_fund_payment` | Positive | Social fund payment |
| `withdrawal` | Negative | Member withdrawal |
| `loan_repayment` | Positive | Loan repayment |
| `fine_payment` | Positive | Fine payment |
| `dividend_payout` | Negative | Dividend distribution |
| `loan_disbursement` | Negative | Loan funds disbursed |

> All amounts are **`i64` integers** in the smallest currency unit (e.g. cents or whole shillings). No floating-point.

---

### Loans — `/api/loans`

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/loans` | List all loans |
| `POST` | `/api/loans` | Request a new loan |
| `GET` | `/api/loans/{id}` | Get loan by ID |
| `GET` | `/api/loans/{id}/guarantors` | List guarantors |
| `POST` | `/api/loans/{id}/guarantors` | Assign a guarantor |
| `POST` | `/api/loans/{id}/approve` | Approve loan (checks pool balance with row lock) |
| `POST` | `/api/loans/{id}/disburse` | Disburse approved loan |
| `GET` | `/api/loans/{id}/schedule` | Amortization schedule (flat & reducing balance) |

**Request loan:**

```json
POST /api/loans
{
  "group_id": "uuid",
  "member_id": "uuid",
  "principal": 100000,
  "term_months": 12
}
```

**Loan lifecycle:** `pending` → `approved` → `disbursed` → `repaid` / `defaulted`

Approval requires at least one guarantor and sufficient group pool balance (`SELECT ... FOR UPDATE`).

---

### Penalties — `/api/penalties`

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/penalties` | List all penalties |
| `GET` | `/api/penalties/{id}` | Get penalty by ID |
| `POST` | `/api/penalties/calculate` | Calculate late loan penalty |
| `POST` | `/api/penalties/apply` | Apply penalty (creates `fine_payment` ledger entry) |

**Calculate penalty:**

```json
POST /api/penalties/calculate
{
  "loan_id": "uuid",
  "overdue_days": 14
}
```

---

### M-Pesa — `/api/mpesa`

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/mpesa/callback` | C2B / STK Push callback webhook |

**Headers:** `x-mpesa-signature` — HMAC-SHA256 of the JSON body, verified against `MPESA_CALLBACK_SECRET`.

**Callback body:**

```json
{
  "transaction_id": "ABC123",
  "phone_number": "254712345678",
  "member_id": "uuid",
  "group_id": "uuid",
  "amount": 50000,
  "result_code": 0,
  "result_desc": "Success"
}
```

On `result_code: 0`, a `deposit` ledger entry is appended automatically.

---

## Financial & Concurrency Rules

### 1. Integer-only money

All financial values (`contributions`, `loans`, `interest`, `balances`, `dividends`, `penalties`) are stored and computed as **`i64`** — the smallest currency unit. No `f64` or `Decimal` anywhere in the hot path.

Interest rates use **basis points** (`bps`): `1200` = 12.00% per annum.

### 2. Append-only ledger

`ledger_transactions` is immutable at the database level (PostgreSQL triggers block `UPDATE` and `DELETE`). The API only exposes `POST` to append entries.

### 3. Transaction safety & row locking

Critical operations run inside SQLx database transactions with explicit row locks:

```sql
SELECT * FROM groups WHERE id = $1 FOR UPDATE
```

This prevents race conditions such as two simultaneous loan approvals depleting the group pool.

### 4. Dividend calculation

Time-weighted proportional method (in `services/finance.rs`):

```
Member Weight = Σ (Contribution Amount × Months Held)
Member Share  = (Member Weight / Total Group Weight) × Distributable Pool
```

### 5. Amortization

Two methods computed per group settings:

- **Flat rate** — total interest = `principal × rate_bps × term_months / 1,200,000`
- **Reducing balance** — standard amortization formula using scaled integer arithmetic

---

## M-Pesa Integration

1. Configure your M-Pesa callback URL to point to:
   ```
   POST https://community-savings-backend.onrender.com/api/mpesa/callback
   ```
2. Set `MPESA_CALLBACK_SECRET` on Render to match your signing key.
3. Include the HMAC-SHA256 hex digest of the raw JSON body in the `x-mpesa-signature` header.

---

## Deployment (Render)

### Build & start commands

| Setting | Value |
|---------|-------|
| **Build command** | `cargo build --release` |
| **Start command** | `./target/release/community-savings-backend` |

### Environment variables (Render dashboard)

| Key | Value |
|-----|-------|
| `DATABASE_URL` | Supabase PostgreSQL connection string (URL-encode password) |
| `MPESA_CALLBACK_SECRET` | Production HMAC secret |

Render sets `PORT` automatically. The app binds to `0.0.0.0:$PORT`.

### Render notes

- Do **not** use `cargo run --release` in production (recompiles on every restart).
- Ensure `DATABASE_URL` uses the Supabase **session pooler** on port `5432` for transaction/`FOR UPDATE` support.

---

## Testing

```bash
# Run all tests
cargo test

# Build release binary
cargo build --release
```

Unit tests in `services/finance.rs` cover flat-rate interest and dividend share distribution.

---

## Security Notes

- **Never commit `.env`** — it is listed in `.gitignore`.
- Store production secrets only in Render environment variables or a secrets manager.
- Rotate database passwords and API secrets if they are ever exposed.
- M-Pesa callbacks are rejected without a valid `x-mpesa-signature` header.
- Use HTTPS in production (Render provides this by default).

---

## License

Private project — all rights reserved.
