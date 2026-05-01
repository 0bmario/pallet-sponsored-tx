# Sequence Diagram

```mermaid
sequenceDiagram
    actor Caller as Signed Caller
    participant Node as Transaction Pool / Node
    participant Ext as SponsoredChargeTransactionPayment EXTENSION
    participant Pallet as Sponsored Tx Pallet
    participant Balances as Balances Pallet (Holds)
    participant Exec as Transaction Execution (Runtime)
    participant Dest as Fee Destination (Treasury/Author)

    Caller->>Node: Submit signed Extrinsic<br/>(sponsor = Some(SponsorAccountId))

    rect rgb(40, 45, 55)
        Note over Node, Pallet: 1. VALIDATE (Read-only Checks)
        Node->>Ext: validate transaction
        Ext->>Pallet: Check Policy
        Pallet-->>Ext: Sponsor active? Caller allowed? Budget sufficient?
    end

    rect rgb(40, 50, 60)
        Note over Ext, Balances: 2. PREPARE (Reservation)
        Ext->>Pallet: move_budget_to_pending(worst_case_fee)
        Pallet->>Balances: release(SponsorshipBudget, worst_case_fee)
        Balances-->>Pallet: Ok
        Pallet->>Balances: hold(SponsorshipPending, worst_case_fee)
        Balances-->>Pallet: Ok
    end

    rect rgb(45, 55, 40)
        Note over Node, Exec: 3. DISPATCH (Execution)
        Node->>Exec: Execute Extrinsic Logic
        Exec-->>Node: Return Actual Consumed Weight / PostDispatchInfo
    end

    rect rgb(55, 45, 40)
        Note over Node, Dest: 4. POST-DISPATCH (Settlement & Refund)
        Node->>Ext: post_dispatch(Actual Weight)
        Ext->>Pallet: Calculate actual fee consumed

        Pallet->>Balances: release & slash(SponsorshipPending, actual_fee)
        Balances-->>Dest: Route actual_fee to FeeDestination

        Pallet->>Balances: release(SponsorshipPending, leftover_fee)
        Balances-->>Pallet: Leftover fee amount

        Pallet->>Balances: hold(SponsorshipBudget, leftover_fee)
        Note right of Balances: Unused reservation is restored<br/>to the sponsor's available budget.
    end
```
