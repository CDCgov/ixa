const { test, expect } = require('@playwright/test');

test('simulation completes successfully', async ({ page }) => {
    await page.goto('http://localhost:8080');

    const result = await page.evaluate(async () => {
        const wasm = await import('/pkg/ixa_wasm_tests.js');
        await wasm.default(); // Ensure WASM module is initialized.
        return await wasm.run_simulation();
    });

    expect(result).toContain('Simulation complete');
});
