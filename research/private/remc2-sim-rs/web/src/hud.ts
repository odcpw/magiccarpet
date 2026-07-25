import type { Simulation } from "./simulation";

const requireElement = <T extends Element>(selector: string): T => {
  const element = document.querySelector<T>(selector);
  if (!element) throw new Error(`Missing required HUD element: ${selector}`);
  return element;
};

export class Hud {
  readonly #progress = requireElement<HTMLElement>("#objective-progress");
  readonly #speedFill = requireElement<HTMLElement>("#speed-fill");
  readonly #altitudeFill = requireElement<HTMLElement>("#altitude-fill");
  readonly #magicFill = requireElement<HTMLElement>("#magic-fill");
  readonly #speedValue = requireElement<HTMLElement>("#speed-value");
  readonly #altitudeValue = requireElement<HTMLElement>("#altitude-value");
  readonly #magicValue = requireElement<HTMLElement>("#magic-value");
  readonly #telemetry = requireElement<HTMLElement>("#telemetry");
  readonly #toast = requireElement<HTMLElement>("#toast");
  readonly #minimap = requireElement<HTMLCanvasElement>("#minimap");
  readonly #mapContext: CanvasRenderingContext2D;
  readonly #mapBase: HTMLCanvasElement;
  #toastTimer = 0;

  constructor(simulation: Simulation) {
    const context = this.#minimap.getContext("2d");
    if (!context) throw new Error("2D minimap context unavailable");
    this.#mapContext = context;
    this.#mapBase = this.#buildMap(simulation);
  }

  update(simulation: Simulation, lit: number, total: number, magic: number): void {
    const speed = Math.abs(simulation.actual_speed());
    const ground = simulation.terrain_height(simulation.x(), simulation.y());
    const altitude = Math.max(0, simulation.z() - ground);
    this.#speedFill.style.transform = `scaleY(${Math.min(1, speed / 80)})`;
    this.#altitudeFill.style.transform = `scaleY(${Math.min(1, altitude / 2400)})`;
    this.#magicFill.style.transform = `scaleY(${Math.max(0, magic / 100)})`;
    this.#speedValue.textContent = String(speed);
    this.#altitudeValue.textContent = String(altitude);
    this.#magicValue.textContent = String(Math.round(magic));
    this.#progress.textContent = `${lit} / ${total} beacons`;
    this.#telemetry.textContent =
      `Rust core · tick ${simulation.tick()} · ${simulation.state_hash_hex().slice(0, 8)}`;
    this.#drawMap(simulation);
  }

  toast(message: string): void {
    this.#toast.textContent = message;
    this.#toast.classList.add("visible");
    window.clearTimeout(this.#toastTimer);
    this.#toastTimer = window.setTimeout(() => {
      this.#toast.classList.remove("visible");
    }, 2_400);
  }

  complete(): void {
    this.#progress.textContent = "Caldera awakened";
    this.#progress.classList.add("complete");
    this.toast("All beacons answer. The caldera is awake.");
  }

  reset(): void {
    this.#progress.classList.remove("complete");
    this.toast("The wind rewinds.");
  }

  #buildMap(simulation: Simulation): HTMLCanvasElement {
    const canvas = document.createElement("canvas");
    canvas.width = 128;
    canvas.height = 128;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Map buffer unavailable");
    const image = context.createImageData(canvas.width, canvas.height);
    for (let y = 0; y < canvas.height; y += 1) {
      for (let x = 0; x < canvas.width; x += 1) {
        const simX = Math.round((x / (canvas.width - 1)) * 65_535);
        const simY = Math.round((y / (canvas.height - 1)) * 65_535);
        const height = simulation.terrain_height(simX, simY);
        const offset = (y * canvas.width + x) * 4;
        const color =
          height < 0
            ? [14, 78, 83]
            : height < 180
              ? [178, 151, 89]
              : height < 1_250
                ? [45, 94, 57]
                : height < 2_100
                  ? [85, 78, 62]
                  : [196, 183, 148];
        image.data[offset] = color[0] ?? 0;
        image.data[offset + 1] = color[1] ?? 0;
        image.data[offset + 2] = color[2] ?? 0;
        image.data[offset + 3] = 255;
      }
    }
    context.putImageData(image, 0, 0);
    return canvas;
  }

  #drawMap(simulation: Simulation): void {
    const context = this.#mapContext;
    const size = this.#minimap.width;
    context.clearRect(0, 0, size, size);
    context.save();
    context.beginPath();
    context.arc(size / 2, size / 2, size * 0.46, 0, Math.PI * 2);
    context.clip();
    context.globalAlpha = 0.86;
    context.drawImage(this.#mapBase, 0, 0, size, size);
    context.globalAlpha = 1;

    const volcanoX = (34_000 / 65_535) * size;
    const volcanoY = (23_000 / 65_535) * size;
    context.fillStyle = "#e38845";
    context.beginPath();
    context.arc(volcanoX, volcanoY, 4, 0, Math.PI * 2);
    context.fill();

    const x = (simulation.x() / 65_535) * size;
    const y = (simulation.y() / 65_535) * size;
    const yaw = (simulation.yaw() / 2_048) * Math.PI * 2;
    context.translate(x, y);
    context.rotate(yaw);
    context.fillStyle = "#f4d58e";
    context.strokeStyle = "#081918";
    context.lineWidth = 2;
    context.beginPath();
    context.moveTo(0, -9);
    context.lineTo(6, 7);
    context.lineTo(0, 4);
    context.lineTo(-6, 7);
    context.closePath();
    context.fill();
    context.stroke();
    context.restore();
  }
}
