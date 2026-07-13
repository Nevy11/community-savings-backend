# Community Savings Backend

This backend powers the Community Savings / Chama platform with a Rust API for member management, contributions, loans, penalties, dividends, and M-Pesa integration.

## What this service does

- Exposes a JSON REST API for the Angular frontend
- Authenticates requests using Supabase JWTs
- Stores user profiles, members, transactions, loans, guarantees, and penalties
- Auto-provisions a backend user profile on `GET /api/users/me` when a confirmed Supabase user has no `user_profiles` row yet
- Provides the profile `role` field used by the frontend to choose between the administrator dashboard and member portal
- Supports group-level financial operations and basic dashboard metrics
- Provides endpoints for M-Pesa STK push and webhook handling
- Dynamically calculates time-weighted dividends based on member contributions

## Tech stack

- Rust 2024 edition
- Axum web framework
- SQLx + PostgreSQL
- Supabase Auth integration
- Serde / JSON

## Project layout

- `src/main.rs` — app entry point and router setup
- `src/handlers/` — HTTP handlers for users, groups, members, transactions, loans, penalties, analytics, and M-Pesa
- `src/models/` — request and response models
- `src/services/` — finance, validation, and payment logic
- `src/middleware.rs` — JWT authentication middleware
- `src/config.rs` — environment and database configuration
- `migrations/` — SQLx database migrations

## Prerequisites

- Rust and Cargo
- A PostgreSQL-compatible database (Supabase is used in this project)
- Environment variables configured in a `.env` file

## Environment variables

Create a `.env` file in the backend root with values like:

```env
DATABASE_URL="postgresql://..."
SUPABASE_JWT_SECRET="your-jwt-secret"
SUPABASE_URL="https://your-project-ref.supabase.co"
SUPABASE_WEBHOOK_SECRET="optional-webhook-secret"
PORT=3000
MPESA_CALLBACK_SECRET="dev-secret"
MPESA_ENVIRONMENT="sandbox"
MPESA_CONSUMER_KEY=""
MPESA_CONSUMER_SECRET=""
MPESA_PASSKEY=""
MPESA_SHORTCODE=""
```

## Run locally

```bash
cargo run
```

Health checks:

```bash
curl http://localhost:3000/ping
curl http://localhost:3000/health
```

## Main API areas

- `/api/users` — profile creation and profile lookup
- `/api/members` — member onboarding, tracking, and attendance/fines
- `/api/transactions` — append-only ledger operations
- `/api/loans` — loan request (with guarantors), approval, and disbursement flow
- `/api/groups` — group settings (interest, fines) and metrics
- `/api/mpesa` — STK push and webhook endpoints
- `/api/analytics` — dividend computation and distribution stats
- `/api/meetings` — bulk attendance tracking and auto-fines

## Financial & Concurrency Rules

1. **Integer-only money:** All financial values (`contributions`, `loans`, `interest`, `balances`, `dividends`, `penalties`) are stored and computed as **`i64`** — the smallest currency unit (cents). No `f64` or `Decimal` anywhere in the hot path. Interest rates use **basis points** (`bps`): `1200` = 12.00% per annum.
2. **Append-only ledger:** `ledger_transactions` is immutable at the database level.
3. **Transaction safety:** Critical operations run inside SQLx database transactions with explicit row locks (`FOR UPDATE`).
4. **Dividends:** Computed server-side using a time-weighted proportional method based on contributions and months held.

## Deployment (Render)

| Setting | Value |
|---------|-------|
| **Build command** | `cargo build --release` |
| **Start command** | `./target/release/community-savings-backend` |

Render sets `PORT` automatically. Ensure `DATABASE_URL` uses the Supabase session pooler on port `5432` for transaction support.

## Supabase Edge Functions

To deploy a Supabase Edge Function, use the following command to ensure JWT verification is handled correctly:

```bash
supabase functions deploy <function_name> --no-verify-jwt
```
