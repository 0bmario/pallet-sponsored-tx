# Security Review

## Scope

This review covers `pallets/sponsored-tx/` (lib.rs, extension.rs, types.rs) and the runtime wiring in `runtime/src/configs/mod.rs`. It focuses on correctness, fund safety, and abuse vectors.

## Severity Scale

- **Critical** — Can cause loss of funds or consensus failure.
- **High** — Can cause incorrect fee accounting or denial of service.
- **Medium** — Edge case that may cause unexpected behavior under specific conditions.
- **Low** — Minor issue, defense-in-depth improvement, or style concern with security implications.
- **Informational** — Not a bug, but worth noting for production readiness.

---

## Findings

### S-01: Non-Atomic Budget-to-Pending Move (Medium)

**Location:** `lib.rs:351-377` (`move_budget_to_pending`)

**Description:** The function performs two separate operations: release from `SponsorshipBudget`, then hold on `SponsorshipPending`. If the second hold fails after the first release succeeds, the funds become free balance rather than held. The sponsor would have unintended free funds, and the transaction would fail with `InvalidTransaction::Payment`.

```rust
pallet_balances::Pallet::<T>::release(&Self::budget_hold_reason(), who, amount, Precision::Exact)
    .map_err(to_validity)?;
pallet_balances::Pallet::<T>::hold(&Self::pending_hold_reason(), who, amount)
    .map_err(to_validity)?;  // if this fails, released funds are free balance
```

**Impact:** The transaction fails, so the signer is not charged and the call is not dispatched. The sponsor ends up with free balance they previously had held, which is a mild invariant violation (budget hold is understated) but not a loss of funds. The sponsor could withdraw those funds before the budget hold is corrected.

**Likelihood:** Low. The hold would only fail if the account cannot support another hold (e.g., it is being reaped simultaneously, or the hold count exceeds `MaxHolds`). In practice with standard `pallet_balances` configuration, this is unlikely.

**Recommendation:** Consider using `transfer_on_hold` or `burn_held` + `hold` patterns that keep funds locked throughout. Alternatively, document this as an accepted edge case for V1.

---

### S-02: Post-Dispatch Error Swallowing in `restore_pending_to_budget` (Medium)

**Location:** `lib.rs:383-427` (`restore_pending_to_budget`)

**Description:** This function runs in `post_dispatch` and cannot return errors. If the release of `SponsorshipPending` succeeds but the re-hold into `SponsorshipBudget` fails, the released funds become free balance. The error is logged but not propagated.

```rust
if let Err(error) = pallet_balances::Pallet::<T>::hold(&Self::budget_hold_reason(), who, released) {
    log::error!(...);  // swallowed
}
```

**Impact:** The sponsor ends up with free balance that should be held as budget. This is a budget accounting inconsistency rather than a loss of funds. The sponsor benefits (unexpected free balance).

**Likelihood:** Same as S-01 — very low under normal conditions.

**Recommendation:** Acceptable for V1 post-dispatch context where errors cannot be propagated. Consider a storage flag or off-chain monitoring to detect this condition in production.

---

### S-03: Fee Clamping in Post-Dispatch (Low)

**Location:** `extension.rs:285-292`

**Description:** If `compute_actual_fee` returns a value larger than the estimated fee (the amount moved to pending), the actual fee is clamped to the estimate. This means the sponsor pays less than the true cost of the transaction.

```rust
if charged_fee_with_tip > estimated_fee_with_tip {
    log::error!(...);
    charged_fee_with_tip = estimated_fee_with_tip;
}
```

**Impact:** The chain absorbs the difference. This should only happen if the fee multiplier increases between validate and post_dispatch (within the same block), which is not possible under normal execution. The clamping is defensive correctness.

**Likelihood:** Extremely low. Fee multiplier is constant within a block.

**Recommendation:** The clamping is the right defensive choice. No change needed.

---

### S-04: No Rate Limiting (Informational)

**Location:** Design scope.

**Description:** A single allowlisted caller can submit as many sponsored transactions as they want, draining the sponsor's entire budget. The only constraints are the per-transaction fee cap and the total budget.

**Impact:** A compromised or malicious allowlisted caller can exhaust sponsor funds quickly. With 6-second block times and low fees, a budget of 2 UNIT could be drained in seconds.

**Recommendation:** This is documented as an intentional V1 scope cut. For production use, consider:
- Per-block or per-epoch rate limits per caller.
- An off-chain monitoring + `pause()` circuit breaker pattern.
- Budget depletion alerts.

---

### S-05: No Call Filtering (Informational)

**Location:** `extension.rs:160-228` (`validate`)

**Description:** The sponsored path does not inspect the call being dispatched. Any extrinsic can be sponsored, including calls to the sponsored-tx pallet itself (e.g., a sponsored `unregister` call), governance calls, or resource-intensive XCM operations.

**Impact:** Low in V1 because the allowlist constrains who can use the sponsor. But an allowlisted caller could submit expensive calls that consume disproportionate budget.

**Recommendation:** Consider whether future versions should support optional pallet/call filters in the policy. For V1, the allowlist provides sufficient control for trusted-caller scenarios.

---

### S-06: O(n^2) Duplicate Check in Policy Validation (Low)

**Location:** `lib.rs:436-444` (`ensure_valid_policy`)

**Description:** The duplicate caller check uses nested iteration:

```rust
for (idx, caller) in policy.allowed_callers.iter().enumerate() {
    if policy.allowed_callers.iter().skip(idx + 1).any(|other| other == caller) {
        return Err(Error::<T>::DuplicateAllowedCaller);
    }
}
```

With `MaxAllowedCallers = 32`, the worst case is ~496 comparisons, which is acceptable. But the algorithm is O(n^2).

**Impact:** None at current limits. If `MaxAllowedCallers` were increased significantly, this could become a weight concern.

**Recommendation:** Acceptable for `n <= 32`. If limits grow, switch to a sort-then-compare or set-based approach.

---

### S-07: `post_dispatch_details` Returned `Weight::zero()` for Sponsored Path (Low) — **Fixed**

**Location:** `extension.rs:331`

**Description:** The sponsored post-dispatch path originally returned `Weight::zero()`, meaning the weight consumed by the settlement logic (slash, restore, event deposit) was not accounted for in the returned weight.

The `weight()` method does add `reads_writes(2, 2)` to the base `ChargeTransactionPayment` weight, which covers validation and prepare. But the post-dispatch overhead is separate.

**Impact:** The block slightly undercharged for sponsored transactions. The difference was small (a few storage reads/writes per sponsored tx).

**Resolution:** `settle_sponsored_fee` now returns a non-zero placeholder settlement weight based on hand-counted storage accesses (`7` reads, `7` writes). This fixes the zero-weight bug while keeping benchmarking as a production-readiness gap. Dedicated benchmarks should still replace the hand-counted values before production.

---

### S-08: `unregister` Releases Budget with `BestEffort` Precision (Low) — **Fixed**

**Location:** `lib.rs:316-323`

**Description:** When a sponsor unregisters, the budget release originally used `Precision::BestEffort` with `let _ = ...?;`. This had two problems: `BestEffort` could silently release less than requested, and the combination of discarding the `Ok` value while propagating errors was misleading.

**Resolution:** Changed to `Precision::Exact` with direct `?` propagation. Since `budget` is read from `balance_on_hold` immediately before, the exact amount must be held — a mismatch would indicate a broken invariant. Using `Exact` ensures unregistration fails rather than silently orphaning held funds by removing the sponsor record while funds remain locked.

```rust
// Release the full budget hold so the sponsor recovers their funds.
// `Exact` is deliberate: `budget` comes from `balance_on_hold` read above, so the
// held amount must match. A mismatch would signal a broken invariant — in that case
// we must fail rather than silently orphan held funds by removing the sponsor record.
let budget = Self::budget_on_hold(&sponsor);
if !budget.is_zero() {
    pallet_balances::Pallet::<T>::release(
        &Self::budget_hold_reason(), &sponsor, budget, Precision::Exact,
    )?;
}
```

---

### S-09: Sponsor Can Be Drained via Tip Inflation (Low)

**Location:** `extension.rs:200-208`

**Description:** The fee cap check includes the tip:

```rust
let fee_with_tip = compute_fee(len, info, self.tip);
if fee_with_tip > state.policy.max_fee_per_tx { ... }
```

An allowlisted caller could set a high tip (up to `max_fee_per_tx`) on every transaction, directing sponsor funds to block authors or fee destinations.

**Impact:** The tip is included in the cap check, so the total cannot exceed `max_fee_per_tx`. But the sponsor may not intend to fund large tips. With `FeeDestination = ()` in the current runtime, tips are burned.

**Recommendation:** Consider whether the policy should have a separate `max_tip` field, or whether `max_fee_per_tx` is sufficient protection. For V1 with trusted callers, this is acceptable.

---

## Summary

| ID | Severity | Title | Status |
|---|---|---|---|
| S-01 | Medium | Non-atomic budget-to-pending move | Accepted for V1 |
| S-02 | Medium | Post-dispatch error swallowing | Accepted for V1 |
| S-03 | Low | Fee clamping in post-dispatch | Correct defensive behavior |
| S-04 | Informational | No rate limiting | Intentional V1 scope cut |
| S-05 | Informational | No call filtering | Intentional V1 scope cut |
| S-06 | Low | O(n^2) duplicate check | Acceptable at n <= 32 |
| S-07 | Low | Zero weight from sponsored post-dispatch | **Fixed** — non-zero placeholder settlement weight |
| S-08 | Low | BestEffort release on unregister | **Fixed** — switched to Exact |
| S-09 | Low | Tip inflation within fee cap | Acceptable with trusted callers |

No critical or high severity issues were found. The medium findings (S-01, S-02) relate to non-atomic hold transitions that are inherent to the current `pallet_balances` hold API and are mitigated by the low likelihood of the failure conditions. The informational findings (S-04, S-05) are documented intentional scope cuts.
