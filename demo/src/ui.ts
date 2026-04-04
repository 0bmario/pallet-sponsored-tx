import {
  registerSponsor,
  increaseBudget,
  decreaseBudget,
  pause,
  resume,
  unregister,
  querySponsorState,
  type SponsorInfo,
} from "./sponsor";
import { submitSponsoredRemark } from "./sponsored-tx";
import { bobAddress, charlieAddress } from "./chain";

const DEV_NAMES: Record<string, string> = {
  [bobAddress]: "Bob",
  [charlieAddress]: "Charlie",
};

// DOM references.
const $ = (id: string) => document.getElementById(id)!;

const connectionStatus = $("connection-status");
const sponsorStatus = $("sponsor-status");
const budgetOnHold = $("budget-on-hold");
const pendingOnHold = $("pending-on-hold");
const aliceBalance = $("alice-balance");
const allowedCallers = $("allowed-callers");
const bobBalance = $("bob-balance");
const eventLog = $("event-log");
const remarkInput = $("remark-input") as HTMLInputElement;

const btnRegister = $("btn-register") as HTMLButtonElement;
const btnIncrease = $("btn-increase") as HTMLButtonElement;
const btnDecrease = $("btn-decrease") as HTMLButtonElement;
const btnPause = $("btn-pause") as HTMLButtonElement;
const btnResume = $("btn-resume") as HTMLButtonElement;
const btnUnregister = $("btn-unregister") as HTMLButtonElement;
const btnSponsoredRemark = $("btn-sponsored-remark") as HTMLButtonElement;

function formatBalance(planck: bigint): string {
  const whole = planck / 1_000_000_000_000n;
  const frac = planck % 1_000_000_000_000n;
  const fracStr = frac.toString().padStart(12, "0").slice(0, 4);
  return `${whole}.${fracStr} UNIT`;
}

function shortAddress(addr: string): string {
  return `${addr.slice(0, 6)}...${addr.slice(-4)}`;
}

function log(message: string, type: "success" | "error" | "info" = "info") {
  const entry = document.createElement("div");
  entry.className = `log-entry ${type}`;
  const time = new Date().toLocaleTimeString();
  entry.innerHTML = `<span class="timestamp">${time}</span>${message}`;
  eventLog.prepend(entry);
}

function updateSponsorButtons(state: SponsorInfo) {
  const registered = state.registered;
  btnRegister.disabled = registered;
  btnIncrease.disabled = !registered;
  btnDecrease.disabled = !registered;
  btnPause.disabled = !registered || !state.active;
  btnResume.disabled = !registered || state.active;
  btnUnregister.disabled = !registered;
  btnSponsoredRemark.disabled = !registered || !state.active;
}

export function updateDisplay(state: SponsorInfo) {
  sponsorStatus.textContent = !state.registered
    ? "Not registered"
    : state.active
      ? "Active"
      : "Paused";

  budgetOnHold.textContent = state.registered ? formatBalance(state.budgetOnHold) : "-";
  pendingOnHold.textContent = state.registered ? formatBalance(state.pendingOnHold) : "-";
  aliceBalance.textContent = formatBalance(state.aliceFreeBalance);
  bobBalance.textContent = formatBalance(state.bobFreeBalance);
  allowedCallers.textContent = state.registered
    ? state.allowedCallers.map((a) => DEV_NAMES[a] ?? shortAddress(a)).join(", ")
    : "-";

  updateSponsorButtons(state);
}

export function setConnected(connected: boolean) {
  connectionStatus.textContent = connected ? "Connected" : "Disconnected";
  connectionStatus.className = `status ${connected ? "connected" : "disconnected"}`;
}

async function withLoading(
  btn: HTMLButtonElement,
  action: () => Promise<string>,
) {
  const original = btn.textContent;
  btn.disabled = true;
  btn.textContent = "...";
  try {
    const msg = await action();
    log(msg, "success");
    // Refresh state after every action.
    const state = await querySponsorState();
    updateDisplay(state);
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    log(message, "error");
    // Still try to refresh state.
    try {
      const state = await querySponsorState();
      updateDisplay(state);
    } catch {
      // ignore refresh failure
    }
  } finally {
    btn.textContent = original;
    // Re-query to get correct button states.
    try {
      const state = await querySponsorState();
      updateSponsorButtons(state);
    } catch {
      // ignore
    }
  }
}

export function bindEvents() {
  btnRegister.addEventListener("click", () =>
    withLoading(btnRegister, registerSponsor),
  );
  btnIncrease.addEventListener("click", () =>
    withLoading(btnIncrease, increaseBudget),
  );
  btnDecrease.addEventListener("click", () =>
    withLoading(btnDecrease, decreaseBudget),
  );
  btnPause.addEventListener("click", () =>
    withLoading(btnPause, pause),
  );
  btnResume.addEventListener("click", () =>
    withLoading(btnResume, resume),
  );
  btnUnregister.addEventListener("click", () =>
    withLoading(btnUnregister, unregister),
  );
  btnSponsoredRemark.addEventListener("click", () =>
    withLoading(btnSponsoredRemark, () =>
      submitSponsoredRemark(remarkInput.value || "Hello from sponsored tx!"),
    ),
  );

  log("UI ready. Click 'Register Alice as Sponsor' to start.", "info");
}
