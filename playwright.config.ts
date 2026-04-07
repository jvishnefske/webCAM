import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  testMatch: '**/*.spec.ts',
  timeout: 30000,
  use: {
    baseURL: 'http://localhost:3000',
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
  webServer: {
    command: 'cargo run -p native-server -- --www-dir www-dataflow --port 3000 --no-open',
    port: 3000,
    reuseExistingServer: true,
    timeout: 60000,
  },
});
