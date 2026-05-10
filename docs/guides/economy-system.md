# Economy System

The economy system gives agents a financial lifecycle: earning wages, buying items, paying employees, and managing budgets. Money flows between agents through the World API — no central bank, just transactions.

---

## Currency

All amounts are in **cents** (integer). Display as dollars: `500 cents = $5.00`.

Every agent has a `balance_cents` field. The city treasury (`city_treasury` agent) holds municipal funds for civic payroll.

---

## Transactions

All money movements are recorded in `economy_transactions`:

| Column | Description |
|--------|-------------|
| `from_agent_id` | Who paid |
| `to_agent_id` | Who received |
| `amount_cents` | How much |
| `reason` | Human-readable description |
| `transaction_type` | `payment`, `salary`, or `purchase` |
| `status` | Always `completed` for now |

---

## Jobs and Wages

### Jobs table

Each job can optionally have an employer and wage:

| Column | Description |
|--------|-------------|
| `employer_id` | Which agent pays for this job (NULL = unpaid) |
| `wage_cents` | Standard wage per pay period |
| `pay_period_minutes` | Minutes between pay periods (default 60) |
| `is_city_job` | If TRUE, city treasury is the implicit employer |
| `max_positions` | Max active employees (NULL = unlimited) |

### Current job roster

| Job | Employer | Wage/hr | City Job | Max Positions |
|-----|----------|---------|----------|---------------|
| Shopkeeper | Self-employed | — | No | — |
| Shop Assistant | Rosie Kim | $5.00 | No | 3 |
| Cafe Owner | Self-employed | — | No | — |
| Mayor | City Treasury | $6.00 | Yes | 1 |
| Groundskeeper | City Treasury | $3.00 | Yes | 2 |
| Librarian | City Treasury | $3.50 | Yes | 1 |
| Clinic Worker | City Treasury | $4.00 | Yes | 2 |
| Student | — | — | No | — |
| Artist | — | — | No | — |
| Music Student | — | — | No | — |
| Professor | — | — | No | — |

### Agent-Jobs lifecycle

Agents hold jobs through the `agent_jobs` table:

| Status | Meaning |
|--------|---------|
| `pending` | Applied, awaiting employer approval |
| `active` | Currently employed |
| `resigned` | Agent quit |
| `fired` | Employer terminated |
| `on_leave` | Reserved for future use |

---

## Employer Tools

Employers (agents with active employees) get these tools:

### `check_payroll`
See all active employees, their wages, when they were last paid, and your total payroll obligation.

### `pay_employee`
Pay an employee from your balance. Creates a salary transaction. **If you can't afford it, the payment is rejected** — bankruptcy is real.

**Parameters:**
- `employee_id` (required) — who to pay
- `amount_cents` (required) — how much
- `reason` (optional) — why

### `fire_employee`
Terminate an employee. They get notified.

**Parameters:**
- `employee_id` (required)
- `reason` (optional)

### `hire_applicant`
Approve a pending job application. Optionally override the default wage.

**Parameters:**
- `applicant_id` (required)
- `job_id` (required)
- `wage_cents` (optional) — override default wage

---

## Employee Tools

### `list_job_openings`
See all available jobs with wages, employers, and how many positions are filled. Always available.

### `apply_for_job`
Apply for a job. City jobs are approved immediately. Private jobs go to `pending` status and the employer is notified.

**Parameters:**
- `job_id` (required)
- `notes` (optional) — pitch to the employer

### `resign_job`
Quit your current primary job. Your employer is notified.

**Parameters:**
- `reason` (optional)

### `collect_city_wage`
City employees collect their salary from the treasury. Only available when enough time has passed since last collection (default: 60 minutes).

---

## Shopkeeper Tools

Shopkeepers at Harvey Oak Supermart get specialized inventory tools:

### `check_shelf_stock`
Audit shelf items, backroom items, pending deliveries, and shop balance.

### `restock_shelf`
Move a backroom item to the aisle shelf and set its retail price.

**Parameters:**
- `item_id` (required) — backroom item to move
- `shelf_price_cents` (required) — retail price

### `order_delivery`
Order new stock. Items arrive as a delivery crate. Cost is deducted from shop balance.

**Parameters:**
- `items` (required) — array of `{name, quantity, consumable_type, vital_value, cost_cents}`

### `receive_delivery`
Unpack a delivery crate into backroom items. Only available when a delivery is pending.

**Parameters:**
- `delivery_id` (required)

### `clean_shop`
Costs 10 stamina. Updates the checkout counter's `last_cleaned_at`.

---

## The Economic Loop

```
1. Rosie orders stock → pays wholesale from her balance
2. Rosie receives delivery → items go to backroom
3. Rosie restocks shelves → items move to aisle with markup
4. Customers buy items → money goes to Rosie's balance
5. Rosie pays Sam → salary deducted from Rosie, credited to Sam
6. If Rosie goes bankrupt → she can't pay Sam → drama
```

City jobs follow a simpler loop:

```
1. Agent applies for city job → auto-approved (if positions available)
2. Agent calls collect_city_wage → treasury pays them
3. Mayor can adjust wages or fire city employees
```

---

## Overdraft Protection

- `pay_agent` and `respond_money_request` reject if balance < amount
- `pay_employee` rejects if employer balance < amount
- Balance CAN go negative via `PATCH /agents/:id/economy` (admin endpoint only)
- This is intentional — admin overrides exist for narrative purposes

---

## Database Schema

### Migration 0012: Job System
Added to `jobs`: `employer_id`, `wage_cents`, `pay_period_minutes`, `is_city_job`
Added to `agent_jobs`: `status`, `hired_at`, `last_paid_at`, `resigned_at`

### Migration 0013: Civic System
Added to `jobs`: `max_positions`
