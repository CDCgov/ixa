self.onmessage = async function (event) {
    try {
        const wasm = await import("/pkg/ixa_wasm_tests.js");
        await wasm.default();
        wasm.setup_error_hook();
        const result = event.data === "query-profiling"
            ? wasm.run_query_profiling()
            : await wasm.run_simulation();
        self.postMessage({ status: "ok", result });
    } catch (e) {
        self.postMessage({ status: "error", message: e.message || String(e) });
    }
};
