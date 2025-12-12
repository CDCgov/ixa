use ixa::log::{debug, error, info, set_log_level, warn};
use ixa::prelude::*;
use ixa::LevelFilter;
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
use web_sys::window;

pub mod infection_manager;
pub mod people;
pub mod transmission_manager;

static POPULATION: u64 = 100;
static SEED: u64 = 123;
static MAX_TIME: f64 = 303.0;
static FOI: f64 = 0.1;
static INFECTION_DURATION: f64 = 5.0;

pub fn initialize(context: &mut Context) {
    context.init_random(SEED);
    people::init(context);
    transmission_manager::init(context);
    infection_manager::init(context);

    context.add_plan(MAX_TIME, |context| {
        context.shutdown();
    });
}

// Ensure that errors are reported in console
#[wasm_bindgen]
pub fn setup_error_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn run_simulation() -> Promise {
    // Wrap our async simulation in a JS Promise
    future_to_promise(async {
        // Start timing
        let performance = window()
            .ok_or("no window object")?
            .performance()
            .ok_or("performance not available")?;
        let start = performance.now();

        // Logging
        set_log_level(LevelFilter::Trace);

        // Actually run the simulation
        let mut context = Context::new();
        initialize(&mut context);
        context.execute();

        let end = performance.now();
        let elapsed = end - start;

        let result = format!("Simulation complete in {:.2} ms", elapsed);

        debug!("This is a debug message.");
        info!("This is an info message.");
        warn!("This is a warning message.");
        error!("This is an error message.");

        Ok(JsValue::from_str(&result))
    })
}

// Simulates a panic by returning a rejected promise.
// Returns rejection instead of a true panic to avoid wasm abort semantics,
// which don't reliably propagate to JS as rejections in all environments.
#[wasm_bindgen]
pub fn run_simulation_panic() -> Promise {
    future_to_promise(async { Err(JsValue::from_str("simulated panic")) })
}

// Triggers a real panic to test the wasm panic hook.
// Must be called synchronously (not awaited) to catch the panic before
// the test framework can suppress it. Uses an index parameter to prevent
// the compiler from detecting the panic statically (which would trigger
// unconditional_panic lint and potentially be optimized away).
#[wasm_bindgen]
pub fn cause_real_panic_with_index(idx: usize) {
    let arr = ["a", "b", "c"];
    // Intentionally access out-of-bounds to trigger panic
    let _ = arr[idx];
}
