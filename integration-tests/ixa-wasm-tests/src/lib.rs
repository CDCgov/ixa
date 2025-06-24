use ixa::LevelFilter;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;
// use gloo_timers::future::TimeoutFuture;
use web_sys::window;
use js_sys::Promise;

use ixa::prelude::*;
use ixa::log::{set_log_level, debug, info, warn, error};

// pub mod incidence_report;
pub mod infection_manager;
pub mod people;
pub mod transmission_manager;

static POPULATION: u64 = 1000;
static SEED: u64 = 123;
static MAX_TIME: f64 = 303.0;
static FOI: f64 = 0.1;
static INFECTION_DURATION: f64 = 5.0;

pub fn initialize(context: &mut Context) {
    context.init_random(SEED);

    people::init(context);
    transmission_manager::init(context);
    infection_manager::init(context);
    // incidence_report::init(context).unwrap_or_else(|e| {
    //     eprintln!("failed to init incidence_report: {}", e);
    // });
    context.add_plan(MAX_TIME, |context| {
        context.shutdown();
    });
}




// Exported to JS
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
        
        // Simulate a 5-second "computation"
        // TimeoutFuture::new(5000).await;
        
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
