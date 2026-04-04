import { connect, disconnect } from "./chain";
import { querySponsorState } from "./sponsor";
import { setConnected, updateDisplay, bindEvents } from "./ui";

async function main() {
  try {
    await connect();
    setConnected(true);

    const state = await querySponsorState();
    updateDisplay(state);

    bindEvents();
  } catch (err) {
    setConnected(false);
    console.error("Failed to connect:", err);
  }
}

main();

// Graceful cleanup on page unload.
window.addEventListener("beforeunload", () => disconnect());
