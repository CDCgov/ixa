use ixa::context::Context;
use ixa::define_data_plugin;
use ixa::report::ContextReportExt;
use ixa::{create_report_trait, report::Report};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

define_data_plugin!(
    SimulationStatePlugin,
    SimulationState,
    SimulationState::new()
);

fn main() {
    let mut context = Context::new();

    context
        .report_options()
        .file_prefix("Reports_".to_string())
        .directory(PathBuf::from("./"));
    context.add_report::<Incidence>("incidence");

    schedule_infection_events(&mut context, 0.5, 1, MAX_DAYS);

    schedule_daily_reports(&mut context, DAILY_REPORT_INTERVAL, 1, MAX_DAYS);

    context.execute();
}

fn schedule_infection_events(
    context: &mut Context,
    interval: f64,
    current_day: u32,
    max_days: u32,
) {
    if current_day >= max_days {
        return;
    }
    context.add_plan(interval, move |context| {
        let sim_state = context.get_data_container_mut(SimulationStatePlugin);
        sim_state.infect_some_people();
        println!(
            "Daily report generated for day {}, Total infected so far: {}",
            current_day, sim_state.total_infected,
        );

        schedule_infection_events(context, interval, current_day + 1, max_days);
    });
}

fn schedule_daily_reports(context: &mut Context, interval: f64, current_day: u32, max_days: u32) {
    if current_day >= max_days {
        return;
    }

    context.add_plan(interval, move |context| {
        let sim_state = context.get_data_container(SimulationStatePlugin).unwrap();
        context.send_report(Incidence {
            day: current_day,
            infections: sim_state.total_infected,
        });
        // Schedule next day's report
        schedule_daily_reports(context, interval, current_day + 1, max_days);
    });
}
