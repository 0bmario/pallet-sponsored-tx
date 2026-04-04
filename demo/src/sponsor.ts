import { getApi, alice, aliceAddress, bobAddress, charlieAddress } from "./chain";
import { SS58String } from "polkadot-api";

const UNIT = 1_000_000_000_000n; // 10^12 planck = 1 DOT/Unit

// Default demo values.
const INITIAL_BUDGET = 2n * UNIT;
const MAX_FEE_PER_TX = UNIT / 2n;
const BUDGET_INCREASE = UNIT / 2n;
const BUDGET_DECREASE = UNIT / 5n;

// Unsponsored txs still need to populate the custom extension (sponsor = undefined → Option::None).
const unsponsoredExt = {
  customSignedExtensions: {
    SponsoredChargeTransactionPayment: {
      value: { tip: 0n, sponsor: undefined },
    },
  },
};

export async function registerSponsor(): Promise<string> {
  const api = getApi();
  const tx = api.tx.SponsoredTx.register_sponsor({
    initial_budget: INITIAL_BUDGET,
    policy: {
      allowed_callers: [bobAddress as SS58String, charlieAddress as SS58String],
      max_fee_per_tx: MAX_FEE_PER_TX,
    },
  });
  const result = await tx.signAndSubmit(alice, unsponsoredExt);
  if (!result.ok) throw new Error(`register_sponsor failed: ${JSON.stringify(result)}`);
  return `Registered Alice as sponsor (budget: 2 UNIT, max fee: 0.5 UNIT)`;
}

export async function increaseBudget(): Promise<string> {
  const api = getApi();
  const tx = api.tx.SponsoredTx.increase_budget({ amount: BUDGET_INCREASE });
  const result = await tx.signAndSubmit(alice, unsponsoredExt);
  if (!result.ok) throw new Error(`increase_budget failed: ${JSON.stringify(result)}`);
  return `Increased budget by 0.5 UNIT`;
}

export async function decreaseBudget(): Promise<string> {
  const api = getApi();
  const tx = api.tx.SponsoredTx.decrease_budget({ amount: BUDGET_DECREASE });
  const result = await tx.signAndSubmit(alice, unsponsoredExt);
  if (!result.ok) throw new Error(`decrease_budget failed: ${JSON.stringify(result)}`);
  return `Decreased budget by 0.2 UNIT`;
}

export async function pause(): Promise<string> {
  const api = getApi();
  const tx = api.tx.SponsoredTx.pause();
  const result = await tx.signAndSubmit(alice, unsponsoredExt);
  if (!result.ok) throw new Error(`pause failed: ${JSON.stringify(result)}`);
  return `Sponsor paused`;
}

export async function resume(): Promise<string> {
  const api = getApi();
  const tx = api.tx.SponsoredTx.resume();
  const result = await tx.signAndSubmit(alice, unsponsoredExt);
  if (!result.ok) throw new Error(`resume failed: ${JSON.stringify(result)}`);
  return `Sponsor resumed`;
}

export async function unregister(): Promise<string> {
  const api = getApi();
  const tx = api.tx.SponsoredTx.unregister();
  const result = await tx.signAndSubmit(alice, unsponsoredExt);
  if (!result.ok) throw new Error(`unregister failed: ${JSON.stringify(result)}`);
  return `Sponsor unregistered`;
}

export interface SponsorInfo {
  registered: boolean;
  active: boolean;
  allowedCallers: string[];
  maxFeePerTx: bigint;
  budgetOnHold: bigint;
  pendingOnHold: bigint;
  aliceFreeBalance: bigint;
  bobFreeBalance: bigint;
}

export async function querySponsorState(): Promise<SponsorInfo> {
  const api = getApi();

  // Query sponsor storage.
  const sponsorState = await api.query.SponsoredTx.Sponsors.getValue(
    aliceAddress as SS58String,
    { at: "best" },
  );

  // Query Alice's account for free balance.
  const aliceAccount = await api.query.System.Account.getValue(
    aliceAddress as SS58String,
    { at: "best" },
  );

  // Query Bob's account for free balance.
  const bobAccount = await api.query.System.Account.getValue(
    bobAddress as SS58String,
    { at: "best" },
  );

  // Query holds on Alice's account for budget and pending amounts.
  const holds = await api.query.Balances.Holds.getValue(
    aliceAddress as SS58String,
    { at: "best" },
  );

  let budgetOnHold = 0n;
  let pendingOnHold = 0n;
  if (holds) {
    for (const hold of holds) {
      const id = hold.id;
      // The hold reason is a runtime enum variant — match on the pallet name.
      if (typeof id === "object" && "type" in id && id.type === "SponsoredTx") {
        const inner = (id as { type: string; value: { type: string } }).value;
        if (inner.type === "SponsorshipBudget") {
          budgetOnHold = hold.amount;
        } else if (inner.type === "SponsorshipPending") {
          pendingOnHold = hold.amount;
        }
      }
    }
  }

  if (!sponsorState) {
    return {
      registered: false,
      active: false,
      allowedCallers: [],
      maxFeePerTx: 0n,
      budgetOnHold,
      pendingOnHold,
      aliceFreeBalance: aliceAccount.data.free,
      bobFreeBalance: bobAccount.data.free,
    };
  }

  return {
    registered: true,
    active: sponsorState.active,
    allowedCallers: sponsorState.policy.allowed_callers.map(String),
    maxFeePerTx: sponsorState.policy.max_fee_per_tx,
    budgetOnHold,
    pendingOnHold,
    aliceFreeBalance: aliceAccount.data.free,
    bobFreeBalance: bobAccount.data.free,
  };
}
