import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  outputDir: "./test-results/artifacts",
  reporter: [["line"]],
  timeout: 90_000,
  use: {
    baseURL: "http://127.0.0.1:8878",
    channel: "chrome",
    headless: false,
    viewport: { width: 960, height: 600 },
    colorScheme: "dark",
    screenshot: "only-on-failure",
    trace: "retain-on-failure"
  },
  webServer: {
    command: "pnpm dev",
    url: "http://127.0.0.1:8878",
    reuseExistingServer: true,
    timeout: 30_000
  }
});
