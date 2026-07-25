export interface Simulation {
  advance(flags: number, roll: number, pitch: number, passes: number): void;
  reset(): void;
  x(): number;
  y(): number;
  z(): number;
  yaw(): number;
  pitch(): number;
  roll(): number;
  target_speed(): number;
  actual_speed(): number;
  strafe_speed(): number;
  tick(): bigint;
  state_hash_hex(): string;
  terrain_height(x: number, y: number): number;
}
