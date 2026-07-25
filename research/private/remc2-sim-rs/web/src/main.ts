import init, { BrowserSimulation } from "../pkg/sim_wasm.js";
import { CarpetGame } from "./game";
import "./styles.css";

const requireElement = <T extends Element>(selector: string): T => {
  const element = document.querySelector<T>(selector);
  if (!element) throw new Error(`Missing required element: ${selector}`);
  return element;
};

const intro = requireElement<HTMLElement>("#intro");
const enterButton = requireElement<HTMLButtonElement>("#enter-button");

const boot = async (): Promise<void> => {
  await init();
  const simulation = new BrowserSimulation();
  const game = new CarpetGame(simulation);

  enterButton.disabled = false;
  enterButton.textContent = "Enter the wind";
  enterButton.addEventListener(
    "click",
    () => {
      intro.classList.add("departing");
      window.setTimeout(() => {
        intro.hidden = true;
      }, 520);
      void game.start();
    },
    { once: true }
  );
};

boot().catch((error: unknown) => {
  console.error("Flight lab failed to start", error);
  enterButton.disabled = true;
  enterButton.textContent = "The wind is unavailable";
  const detail = document.createElement("p");
  detail.className = "boot-error";
  detail.textContent = error instanceof Error ? error.message : String(error);
  enterButton.insertAdjacentElement("afterend", detail);
});
