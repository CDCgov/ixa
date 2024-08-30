use ixa::context::Context;
use ixa::report::ContextReportExt;
use ixa::{create_report_trait, report::Report};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Serialize, Deserialize, Clone)]
struct Incidence {
    day: u32,
    infections: usize,
}

create_report_trait!(Incidence);

static MAX_DAYS: u32 = 14;
static DAILY_REPORT_INTERVAL: f64 = 1.0; // 1.0 = one day in simulation time

struct SimulationState {
    total_infected: usize,
}

impl SimulationState {
    fn new() -> Self {
        SimulationState { total_infected: 0 }
    }

    fn infect_some_people(&mut self) {
        self.total_infected += 5;
    }
}

fn main() {
    let mut context = Context::new();
    let sim_state = Rc::new(RefCell::new(SimulationState::new()));

    context
        .report_options()
        .file_prefix("Reports_".to_string())
        .directory(PathBuf::from("./"));
    context.add_report::<Incidence>("incidence");

    schedule_infection_events(&mut context, 0.5, 1, MAX_DAYS, Rc::clone(&sim_state));

    schedule_daily_reports(
        &mut context,
        DAILY_REPORT_INTERVAL,
        1,
        MAX_DAYS,
        Rc::clone(&sim_state),
    );

    context.execute();
}

fn schedule_infection_events(
    context: &mut Context,
    interval: f64,
    current_day: u32,
    max_days: u32,
    sim_state: Rc<RefCell<SimulationState>>,
) {
    if current_day >= max_days {
        return;
    }

    context.add_plan(interval, move |context| {
        sim_state.borrow_mut().infect_some_people();
        println!(
            "Daily report generated for day {}, Total infected so far: {}",
            current_day,
            sim_state.borrow().total_infected,
        );

        schedule_infection_events(
            context,
            interval,
            current_day + 1,
            max_days,
            Rc::clone(&sim_state),
        );
    });
}

fn schedule_daily_reports(
    context: &mut Context,
    interval: f64,
    current_day: u32,
    max_days: u32,
    sim_state: Rc<RefCell<SimulationState>>,
) {
    if current_day >= max_days {
        return;
    }

    context.add_plan(interval, move |context| {
        generate_daily_report(context, current_day, &Rc::clone(&sim_state));

        // Schedule next day's report
        schedule_daily_reports(
            context,
            interval,
            current_day + 1,
            max_days,
            Rc::clone(&sim_state),
        );
    });
}

fn generate_daily_report(
    context: &mut Context,
    day: u32,
    sim_state: &Rc<RefCell<SimulationState>>,
) {
    context.send_report(Incidence {
        day,
        infections: sim_state.borrow().total_infected,
    });
}
