const { test, expect } = require('@playwright/test');

test('simulation completes successfully', async ({ page }) => {
    await page.goto('http://localhost:8080');

    const output = page.locator('#output');

    await expect(output).toHaveText(/Running simulation.../);

    // Wait up to 10 seconds for the simulation to complete
    await expect(output).toHaveText(/Simulation complete/i, { timeout: 10000 });
});
