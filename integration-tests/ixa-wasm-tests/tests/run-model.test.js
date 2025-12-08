const { test, expect } = require('@playwright/test');

test.beforeEach(async ({ page }) => {
    await page.addInitScript(async () => {
        window.setupWasm = async () => {
            const wasm = await import('/pkg/ixa_wasm_tests.js');
            await wasm.default();
            wasm.setup_error_hook();
            return wasm;
        };
    });
});

test('simulation completes successfully', async ({ page }) => {
    await page.goto('http://localhost:8080');

    const result = await page.evaluate(async () => {
        let wasm = await window.setupWasm();
        return await wasm.run_simulation();
    });

    expect(result).toContain('Simulation complete');
});

test('simulation error (simulated panic) as expected', async ({ page }) => {
    await page.goto('http://localhost:8080');

    const result = await page.evaluate(async () => {
        let wasm = await window.setupWasm();
        try {
            await wasm.run_simulation_panic();
            return { status: 'resolved' }; // Should never reach here; promise rejects
        } catch (e) {
            return { status: 'error', message: (e && e.message) ? e.message : String(e) };
        }
    });

    // Verify the promise rejection was caught. Don't assert on message content
    // as it varies across environments and build configurations.
    expect(result.status).toBe('error');
});

test('real wasm panic emits console error', async ({ page }) => {
    const consoleMessages = [];
    page.on('console', msg => consoleMessages.push(msg.text()));
    const pageErrors = [];
    page.on('pageerror', err => pageErrors.push(err.message));

    await page.goto('http://localhost:8080');

    // Trigger the panic synchronously (not awaited) so the panic hook
    // can output to console before the promise rejection is handled.
    await page.evaluate(() => {
        window.setupWasm().then(wasm => {
            wasm.cause_real_panic_with_index(4); // Pass out-of-bounds index
        });
    });

    // Wait up to 5 seconds for panic output in console or page errors.
    const detected = await Promise.race([
        page.waitForEvent('console', {
            timeout: 5000,
            predicate: m => m.text().includes('index out of bounds') || m.text().includes('panicked')
        }).then(() => true).catch(() => false),
        (async () => {
            const start = Date.now();
            while (Date.now() - start < 5000) {
                if (consoleMessages.some(m => m.includes('index out of bounds') || m.includes('panicked')) ||
                    pageErrors.some(e => e.includes('index out of bounds') || e.includes('panicked'))) {
                    return true;
                }
                await new Promise(r => setTimeout(r, 100));
            }
            return false;
        })()
    ]);

    if (!detected) {
        // eslint-disable-next-line no-console
        console.log('Panic not detected. Console messages:', consoleMessages, 'Page errors:', pageErrors);
    }
    expect(detected).toBeTruthy();
});
