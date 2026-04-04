import { getApi, bob, aliceAddress } from "./chain";
import { Binary, SS58String } from "polkadot-api";

/**
 * Submit a System::remark extrinsic where Alice (sponsor) pays the fees
 * instead of Bob (signer).
 *
 * This uses PAPI's `customSignedExtensions` to populate the
 * `SponsoredChargeTransactionPayment` extension with `{ tip, sponsor }`.
 */
export async function submitSponsoredRemark(message: string): Promise<string> {
  const api = getApi();

  const tx = api.tx.System.remark({ remark: Binary.fromText(message) });

  const result = await tx.signAndSubmit(bob, {
    customSignedExtensions: {
      SponsoredChargeTransactionPayment: {
        value: {
          tip: 0n,
          sponsor: aliceAddress as SS58String,
        },
      },
    },
  });

  if (!result.ok) {
    throw new Error(`Sponsored remark failed: ${JSON.stringify(result)}`);
  }

  // Look for the SponsoredTransactionFeePaid event in the result.
  for (const event of result.events) {
    if (
      event.type === "SponsoredTx" &&
      event.value.type === "SponsoredTransactionFeePaid"
    ) {
      const { actual_fee, tip } = event.value.value;
      return `Sponsored remark submitted! Fee: ${formatBalance(actual_fee)}, Tip: ${formatBalance(tip)}`;
    }
  }

  return `Sponsored remark submitted (tx hash: ${result.txHash})`;
}

function formatBalance(planck: bigint): string {
  const whole = planck / 1_000_000_000_000n;
  const frac = planck % 1_000_000_000_000n;
  const fracStr = frac.toString().padStart(12, "0").replace(/0+$/, "");
  return fracStr ? `${whole}.${fracStr} UNIT` : `${whole} UNIT`;
}
