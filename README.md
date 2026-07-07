# Community Savings Backend

This backend powers the Community Savings / Chama platform with a Rust API for member management, contributions, loans, penalties, and M-Pesa integration.

## What this service does

- Exposes a JSON API for the Angular frontend
- Authenticates requests using Supabase JWTs
- Stores user profiles, members, transactions, loans, and penalties
- Auto-provisions a backend user profile on `GET /api/users/me` when a confirmed Supabase user has no `user_profiles` row yet
- Provides the profile `role` field used by the frontend to choose between the administrator dashboard and member portal
- Supports group-level financial operations and basic dashboard metrics
- Provides endpoints for M-Pesa STK push and webhook handling

## Tech stack

- Rust 2024 edition
- Axum web framework
- SQLx + PostgreSQL
- Supabase Auth integration
- Serde / JSON

## Project layout

- src/main.rs — app entry point and router setup
- src/handlers/ — HTTP handlers for users, groups, members, transactions, loans, penalties, and M-Pesa
- src/models/ — request and response models
- src/services/ — finance, validation, and payment logic
- src/middleware.rs — JWT authentication middleware
- src/config.rs — environment and database configuration

## Prerequisites

- Rust and Cargo
- A PostgreSQL-compatible database (Supabase is used in this project)
- Environment variables configured in a .env file

## Environment variables

Create a .env file in the backend root with values like:

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

- /api/users — profile creation and profile lookup
- /api/members — member onboarding and attendance
- /api/transactions — append-only ledger operations
- /api/loans — loan request, approval, and disbursement flow
- /api/groups — group settings and dashboard metrics
- /api/mpesa — STK push and webhook endpoints

## Frontend role contract

The Angular frontend uses the authenticated user's profile role to select the interface:

- `administrator` — sees the admin console with overview, members, transactions, loans, and dividends analytics.
- `member` — sees the member portal with personal savings, loan balances, fines, meeting information, guarantees, and statements.

New profiles default to `member` unless the database or profile sync explicitly assigns `administrator`.

## Authentication

Protected routes expect a bearer token:

```http
Authorization: Bearer <supabase-access-token>
```

The backend validates the token and uses the Supabase user id from the JWT subject claim.

`jsonwebtoken` 10 is built with the `rust_crypto` backend and the provider is installed during startup. This is required before validating Supabase JWTs.

## Testing

```bash
cargo test
```

## Notes

- The frontend currently calls the backend through the /api routes.
- The backend is designed to work with the deployed Render instance and the Supabase PostgreSQL database.


Most `/api/*` routes are **protected** and require a valid Supabase access token:

```
Authorization: Bearer <supabase_access_token>
```

The middleware validates the JWT against `SUPABASE_JWT_SECRET` and extracts the authenticated user's ID (`sub`), email, and metadata (including `full_name` from `user_metadata`).

**Public routes** (no auth required):
- `GET /ping`
- `GET /health`
- `POST /api/mpesa/callback`

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

### User Profiles — `/api/users`

Requires `Authorization: Bearer <token>`.

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/users/profile` | Create or sync profile after Supabase sign-in |
| `GET` | `/api/users/profile/{username}` | Fetch profile by username |
| `GET` | `/api/users/me` | Fetch current user's profile by JWT `sub`; creates one from JWT metadata/email if missing |
| `PATCH` | `/api/users/profile` | Update profile (theme, full name) |

**Profile response shape:**

```json
{
  "id": "uuid",
  "username": "jane_doe",
  "full_name": "Jane Doe",
  "preferred_theme": "light",
  "role": "member",
  "email": "jane@example.com"
}
```

**Create / sync profile after sign-in:**

```json
POST /api/users/profile
{
  "username": "jane_doe",
  "full_name": "Jane Doe"
}
```

- `username` — required (3–32 chars; letters, numbers, `_`, `.`)
- `full_name` — optional; if omitted, read from JWT `user_metadata.full_name` or `user_metadata.name`

When `/api/users/me` auto-creates a profile, it derives `username` from `user_metadata.username` first, then the email local-part, then a `user_<auth-id>` fallback. The derived username is normalized to the same character rules used by explicit profile creation.

**Fetch profile by username:**

```
GET /api/users/profile/jane_doe
```

Access rules:
- Users can fetch their **own** profile
- Users with `role: "administrator"` can fetch **any** profile

**Update profile (e.g. theme toggle):**

```json
PATCH /api/users/profile
{
  "preferred_theme": "dark",
  "full_name": "Jane M. Doe"
}
```

Both fields are optional — send only what you want to change.

**Defaults for new profiles:**
- `preferred_theme` → `"light"`
- `role` → `"member"`

Accepted role values:
- `"member"`
- `"administrator"`

**Frontend flow:**

1. User signs in via Supabase Auth on the client
2. Call `POST /api/users/profile` with `username` (and optionally `full_name`)
3. On app load, call `GET /api/users/profile/{username}` or `GET /api/users/me`
4. Apply `preferred_theme` and route the user based on `role`
5. Use `PATCH /api/users/profile` when the user changes theme settings

**Example (JavaScript):**

```javascript
const { data: { session } } = await supabase.auth.getSession();
const token = session.access_token;

// Sync profile after sign-in
await fetch(`${API_BASE}/api/users/profile`, {
  method: 'POST',
  headers: {
    Authorization: `Bearer ${token}`,
    'Content-Type': 'application/json',
  },
  body: JSON.stringify({
    username: 'jane_doe',
    full_name: session.user.user_metadata?.full_name,
  }),
});

// Fetch profile
const res = await fetch(`${API_BASE}/api/users/profile/jane_doe`, {
  headers: { Authorization: `Bearer ${token}` },
});
const profile = await res.json();
```

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
| `SUPABASE_JWT_SECRET` | Supabase JWT secret (Project Settings → API) |
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
- Protected API routes require a valid Supabase JWT; unauthenticated requests receive `401`.
- M-Pesa callbacks are rejected without a valid `x-mpesa-signature` header.
- Use HTTPS in production (Render provides this by default).

---

## License

Private project — all rights reserved.
