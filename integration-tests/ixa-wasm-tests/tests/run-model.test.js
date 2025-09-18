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

test('simulation panics as expected', async ({ page }) => {
    // Capture browser console messages
    const messages = [];
    page.on('console', msg => {
        messages.push({ type: msg.type(), text: msg.text() });
    });

    await page.goto('http://localhost:8080');

    // Run the panic function with a timeout to detect a hang/crash
    const errorInfo = await page.evaluate(async () => {
        const wasm = await import('/pkg/ixa_wasm_tests.js');
        await wasm.default();
        function withTimeout(promise, ms) {
            return Promise.race([
                promise,
                new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), ms))
            ]);
        }
        try {
            await withTimeout(wasm.run_simulation_panic(), 2000);
            return { result: 'resolved' };
        } catch (e) {
            let message = (e && e.message) ? e.message : String(e);
            let stack = e && e.stack ? e.stack : null;
            return { result: message, raw: e, stack };
        }
    });

    // The panic should either cause a timeout or appear in the browser console
    const hasPanicMsg = messages.some(m => m.text.includes('panicked') || m.text.includes('index out of bounds'));
    // Print any message that contains 'index out of bounds'
    messages.filter(m => m.text.includes('index out of bounds')).forEach(m => {
        // eslint-disable-next-line no-console
        console.log('Captured panic message:', m.text);
    });
    expect(errorInfo.result === 'timeout' || hasPanicMsg).toBeTruthy();
});
