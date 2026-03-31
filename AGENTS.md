# AGENTS.md

FRAME pallet for sponsor-paid transaction fees on Polkadot SDK. Learning project focused on the `TransactionExtension` lifecycle.

## Key design decisions

- **TransactionExtension pattern**: `SponsoredChargeTransactionPayment` wraps `ChargeTransactionPayment`, delegating to it for unsponsored txs.
- **Two-hold model**: sponsor funds are held under `SponsorshipBudget` (long-lived) and `SponsorshipPending` (per-tx lifetime). `move_budget_to_pending` in prepare, `restore_pending_to_budget` in post_dispatch.
- **V1 scope**: trusted callers only (allowlist), no rate limiting, no call filtering. See `docs/security_review.md` S-04/S-05.

## Reviewing and writing code

- Formatting requires nightly: `just fmt` (or `cargo +nightly fmt`).
- `docs/security_review.md` is the source of truth for known issues and accepted trade-offs.
- Hold accounting is the critical invariant — any change to hold/release/slash flows must be reviewed against S-01 and S-02.
- Weights: the sponsored post-dispatch path returns `Weight::zero()` (S-07) — fix before production.
- After any change to pallet logic, hold flows, extension behavior, or security properties, update the relevant doc in `docs/` (especially `security_review.md`). Docs must stay in sync with code.
- If you discover a new critical design decision, invariant, or footgun, add it to this AGENTS.md so future sessions benefit.
