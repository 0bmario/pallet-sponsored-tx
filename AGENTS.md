# AGENTS.md

FRAME pallet for sponsor-paid transaction fees on Polkadot SDK. Learning project focused on the `TransactionExtension` lifecycle.

## Key design decisions

- **TransactionExtension pattern**: `SponsoredChargeTransactionPayment` wraps `ChargeTransactionPayment`, delegating to it for unsponsored txs.
- **Two-hold model**: sponsor funds are held under `SponsorshipBudget` (long-lived) and `SponsorshipPending` (per-tx lifetime). `move_budget_to_pending` in prepare, `restore_pending_to_budget` in post_dispatch.
- **V1 scope**: trusted callers only (allowlist), no rate limiting, no call filtering. See `docs/security_review.md` S-04/S-05.

## Reviewing and writing code

- `docs/security_review.md` is the source of truth for known issues and accepted trade-offs.
- Hold accounting is the critical invariant — any change to hold/release/slash flows must be reviewed against S-01 and S-02.
- Weights: dispatchable weights in `weights.rs` are benchmark-derived; the sponsored post-dispatch extension path still uses a non-zero placeholder DB weight until dedicated extension benchmarks exist.
- Regenerate dispatchable weights with `just benchmark-sponsored-tx`. Always use the repo-local `.maintain/frame-weight-template.hbs` template so `weights.rs` keeps the local `WeightInfo` / `SubstrateWeight<T>` / `impl WeightInfo for ()` contract.
- Benchmark-enabled node builds intentionally set `SKIP_PALLET_REVIVE_FIXTURES=1` so unrelated `pallet-revive` fixture generation does not introduce a `solc` requirement into this repo's benchmarking flow.
- After any change to pallet logic, hold flows, extension behavior, or security properties, update the relevant doc in `docs/` (especially `security_review.md`). Docs must stay in sync with code.
- If you discover a new critical design decision, invariant, or footgun, add it to this AGENTS.md so future sessions benefit.
