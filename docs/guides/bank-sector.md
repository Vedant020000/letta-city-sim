# Bank Sector

The bank sector gives town money somewhere useful to flow. Agents can hold cash, move money into a bank deposit account, borrow money, and repay loans. A banker chooses deposit and loan rates separately.

The core daily accounting rule is:

```text
deposit_balance_tomorrow = deposit_balance_today * (1 + daily_deposit_rate)
loan_balance_tomorrow    = loan_balance_today    * (1 + daily_loan_rate)
```

## Cash vs deposits

`agents.balance_cents` remains cash on hand. Agents spend from this balance when buying items, paying other agents, or repaying loans.

Bank deposits live in `bank_accounts`. Deposits are not cash on hand. To spend deposited money, the agent first withdraws it.

This distinction gives the bank sector its role:

```text
cash -> deposit account -> lendable bank reserves -> loans -> cash
```

## Rates

`banks.deposit_rate_daily` controls deposit interest.

`banks.loan_rate_daily` controls new loan interest. Loans lock the current loan rate at origination, so future banker rate changes do not mutate older loans.

Rates are guarded:

- rates cannot be negative
- rates cannot exceed 5% daily
- loan rate must be at least the deposit rate

## Lazy interest accrual

Interest accrues lazily when an account or loan is touched.

Actions that inspect or mutate an account first compute simulated elapsed days from `last_accrued_at`, compound the balance, round to cents, write a ledger entry for the interest delta, and update `last_accrued_at`.

No background job is required.

## Agent tools

### `check_bank_rates`

Returns the bank location, daily deposit rate, daily loan rate, approximate APYs, and reserve ratio.

### `check_bank_account`

Returns the agent's cash balance, deposit balance, active loans, total outstanding loan balance, and current bank deposit rate.

### `deposit_money`

Moves cash into the agent's bank account.

```json
{ "amount_cents": 500 }
```

Rejects if the agent does not have enough cash.

### `withdraw_money`

Moves money from the agent's bank account back to cash.

```json
{ "amount_cents": 500 }
```

Rejects if the deposit balance is too low.

### `take_loan`

Creates a new active loan and adds the principal to cash.

```json
{
  "amount_cents": 2000,
  "purpose": "buy class supplies"
}
```

V1 limits:

- an agent's active loans plus requested amount cannot exceed 10,000 cents
- the bank must have enough lendable funds after reserves

### `repay_loan`

Repays part or all of an active loan from cash.

```json
{
  "loan_id": "loan_...",
  "amount_cents": 500
}
```

If the payment exceeds the outstanding balance, only the outstanding balance is applied. The loan is marked `paid` when it reaches zero.

## Banker tools

Banker tools are available only to agents with an active `banker` job.

### `set_bank_rates`

Sets deposit and loan rates separately.

```json
{
  "deposit_rate_daily": 0.0005,
  "loan_rate_daily": 0.002
}
```

### `check_bank_balance_sheet`

Returns bank cash, total deposits, outstanding loans, reserve requirement, available lendable funds, and current rates.

## Reserve rule

V1 uses a simple reserve rule:

```text
available_to_lend = bank_balance_cents - ceil(total_deposits_cents * reserve_ratio)
```

Deposits increase `bank_balance_cents`; withdrawals and loan disbursements reduce it; repayments increase it.

This is intentionally simple. It gives deposits a clear role without trying to model a full banking system.

## Seeded bank sector

Locations:

- `smallville_bank_lobby`
- `smallville_bank_office`
- `smallville_bank_vault`

Seeded entities:

- `smallville_bank`
- `bank_treasury`
- `nora_patel`, banker

Jobs:

- `banker`
- `bank_teller`

## Ledger

Every bank money movement is written to `bank_ledger_entries`:

- `deposit`
- `withdrawal`
- `deposit_interest`
- `loan_disbursement`
- `loan_interest`
- `loan_repayment`
- `rate_change`

Use the ledger when debugging money supply behavior.
