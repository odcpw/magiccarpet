export interface FlightCommand {
  flags: number;
  roll: number;
  pitch: number;
}

const clamp = (value: number, minimum: number, maximum: number): number =>
  Math.max(minimum, Math.min(maximum, value));

export class FlightInput {
  readonly #canvas: HTMLCanvasElement;
  readonly #keys = new Set<string>();
  readonly #onFire: () => void;
  readonly #onReset: () => void;
  readonly #onPause: () => void;
  readonly #allowPointerLock = !new URLSearchParams(window.location.search).has(
    "automation"
  );
  #roll = 0;
  #pitch = 0;
  #enabled = false;

  constructor(
    canvas: HTMLCanvasElement,
    onFire: () => void,
    onReset: () => void,
    onPause: () => void
  ) {
    this.#canvas = canvas;
    this.#onFire = onFire;
    this.#onReset = onReset;
    this.#onPause = onPause;

    window.addEventListener("keydown", this.#handleKeyDown, { passive: false });
    window.addEventListener("keyup", this.#handleKeyUp);
    window.addEventListener("blur", () => this.#keys.clear());
    document.addEventListener("mousemove", this.#handleMouseMove);
    this.#canvas.addEventListener("click", () => {
      if (
        this.#enabled
        && this.#allowPointerLock
        && document.pointerLockElement !== this.#canvas
      ) {
        void this.#canvas.requestPointerLock();
      }
    });
  }

  enable(): void {
    this.#enabled = true;
    if (this.#allowPointerLock) void this.#canvas.requestPointerLock();
  }

  sample(): FlightCommand {
    let flags = 0;
    if (this.#keys.has("KeyW")) flags |= 0b0001;
    if (this.#keys.has("KeyS")) flags |= 0b0010;
    if (this.#keys.has("KeyA")) flags |= 0b0100;
    if (this.#keys.has("KeyD")) flags |= 0b1000;

    const keyRoll = (this.#keys.has("ArrowRight") || this.#keys.has("KeyE") ? 28 : 0)
      - (this.#keys.has("ArrowLeft") || this.#keys.has("KeyQ") ? 28 : 0);
    const keyPitch = (this.#keys.has("ArrowDown") ? 22 : 0)
      - (this.#keys.has("ArrowUp") ? 22 : 0);

    const command = {
      flags,
      roll: Math.round(clamp(this.#roll + keyRoll, -64, 64)),
      pitch: Math.round(clamp(this.#pitch + keyPitch, -64, 64))
    };
    this.#roll *= 0.78;
    this.#pitch *= 0.8;
    if (Math.abs(this.#roll) < 0.2) this.#roll = 0;
    if (Math.abs(this.#pitch) < 0.2) this.#pitch = 0;
    return command;
  }

  readonly #handleMouseMove = (event: MouseEvent): void => {
    if (!this.#enabled || document.pointerLockElement !== this.#canvas) return;
    this.#roll = clamp(this.#roll + event.movementX * 0.42, -64, 64);
    this.#pitch = clamp(this.#pitch + event.movementY * 0.34, -64, 64);
  };

  readonly #handleKeyDown = (event: KeyboardEvent): void => {
    if (
      ["Space", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.code)
    ) {
      event.preventDefault();
    }
    if (event.repeat && ["Space", "KeyR", "KeyP"].includes(event.code)) return;
    this.#keys.add(event.code);
    if (!this.#enabled) return;
    if (event.code === "Space") this.#onFire();
    if (event.code === "KeyR") this.#onReset();
    if (event.code === "KeyP") this.#onPause();
  };

  readonly #handleKeyUp = (event: KeyboardEvent): void => {
    this.#keys.delete(event.code);
  };
}
