const { test, expect } = require('@playwright/test');

const TEST_PAGE = 'http://localhost:8080/test-harness.html';
const CONSOLE_EVENT_TIMEOUT_MS = 5_000;

test.beforeEach(async ({ page }) => {
    await page.addInitScript(async () => {
        window.setupWasm = async (packagePath = '/pkg/ixa_wasm_tests.js') => {
            const wasm = await import(packagePath);
            await wasm.default();
            wasm.setup_error_hook();
            return wasm;
        };
    });
});

const waitForProfilingTable = page => Promise.all([
    page.waitForEvent('console', {
        predicate: message =>
            message.text().includes('Query') && message.text().includes('Count'),
        timeout: CONSOLE_EVENT_TIMEOUT_MS,
    }),
    page.waitForEvent('console', {
        predicate: message => message.text().includes('Person: (InfectionStatus)'),
        timeout: CONSOLE_EVENT_TIMEOUT_MS,
    }),
]);

test('simulation completes successfully', async ({ page }) => {
    await page.goto(TEST_PAGE);

    const result = await page.evaluate(async () => {
        let wasm = await window.setupWasm();
        return await wasm.run_simulation();
    });

    expect(result).toContain('Simulation complete');
});

test('logging works with only the logging feature enabled', async ({ page }) => {
    await page.goto(TEST_PAGE);

    const expectedMessages = [
        'This is a debug message.',
        'This is an info message.',
        'This is a warning message.',
        'This is an error message.',
    ];
    const loggingOutput = Promise.all(expectedMessages.map(expectedMessage =>
        page.waitForEvent('console', {
            predicate: message => message.text().includes(expectedMessage),
            timeout: CONSOLE_EVENT_TIMEOUT_MS,
        })
    ));
    const [result] = await Promise.all([
        page.evaluate(async () => {
            const wasm = await window.setupWasm('/pkg/logging/ixa_wasm_tests.js');
            return wasm.run_simulation();
        }),
        loggingOutput,
    ]);

    expect(result).toContain('Simulation complete');
});

test('profiling works with only the profiling feature enabled', async ({ page }) => {
    await page.goto(TEST_PAGE);

    const profilingOutput = waitForProfilingTable(page);
    const [result] = await Promise.all([
        page.evaluate(async () => {
            const wasm = await window.setupWasm('/pkg/profiling/ixa_wasm_tests.js');
            return wasm.run_query_profiling();
        }),
        profilingOutput,
    ]);

    expect(result).toBe(100);
});

test('simulation error (simulated panic) as expected', async ({ page }) => {
    await page.goto(TEST_PAGE);

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

test('simulation completes successfully in a web worker', async ({ page }) => {
    await page.goto(TEST_PAGE);

    const result = await page.evaluate(async () => {
        return new Promise((resolve, reject) => {
            const worker = new Worker('/worker.js', { type: 'module' });
            worker.onmessage = (e) => {
                worker.terminate();
                if (e.data.status === 'ok') {
                    resolve(e.data.result);
                } else {
                    reject(new Error(e.data.message));
                }
            };
            worker.onerror = (e) => {
                worker.terminate();
                reject(new Error(e.message));
            };
            worker.postMessage('start');
        });
    });

    expect(result).toContain('Simulation complete');
});

test('query profiling prints in the browser and a web worker', async ({ page }) => {
    await page.goto(TEST_PAGE);

    const browserOutput = waitForProfilingTable(page);
    const [browserResult] = await Promise.all([
        page.evaluate(async () => {
            const wasm = await window.setupWasm();
            return wasm.run_query_profiling();
        }),
        browserOutput,
    ]);

    const workerController = await page.evaluateHandle(() => {
        const worker = new Worker('/worker.js', { type: 'module' });
        const result = new Promise((resolve, reject) => {
            worker.onmessage = (e) => {
                if (e.data.status === 'ok') {
                    resolve(e.data.result);
                } else {
                    reject(new Error(e.data.message));
                }
            };
            worker.onerror = (e) => reject(new Error(e.message));
        });

        return { worker, result };
    });
    const workerOutput = waitForProfilingTable(page);
    let workerResult;

    try {
        [workerResult] = await Promise.all([
            workerController.evaluate(({ worker, result }) => {
                worker.postMessage('query-profiling');
                return result;
            }),
            workerOutput,
        ]);
    } finally {
        await workerController.evaluate(({ worker }) => worker.terminate());
        await workerController.dispose();
    }

    expect(browserResult).toBe(100);
    expect(workerResult).toBe(100);
});

test('real wasm panic emits console error', async ({ page }) => {
    const consoleMessages = [];
    page.on('console', msg => consoleMessages.push(msg.text()));
    const pageErrors = [];
    page.on('pageerror', err => pageErrors.push(err.message));

    await page.goto(TEST_PAGE);

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
