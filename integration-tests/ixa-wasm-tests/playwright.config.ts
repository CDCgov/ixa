import { defineConfig } from '@playwright/test';

export default defineConfig({
    // Run your local dev server before starting the tests
    webServer: {
        command: 'npm run start',
        url: 'http://localhost:8080',
        timeout: 10_000,
        stdout: 'ignore',
        stderr: 'pipe',
        gracefulShutdown: { signal: 'SIGTERM', timeout: 500 },
    },
});
