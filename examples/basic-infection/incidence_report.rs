use ixa::context::Context;

use crate::people::InfectionStatus;
use crate::people::InfectionStatusEvent;

struct IncidenceReportItem {
    time: f64,
    person_id: usize,
    infection_status: InfectionStatus,
}

fn handle_infection_status_change(context: &mut Context, event: InfectionStatusEvent) {
    let report: IncidenceReportItem = IncidenceReportItem {
        time: context.get_current_time(),
        person_id: event.person_id,
        infection_status: event.updated_status,
    };
    println!(
        "{:?}, {:?}, {:?}",
        report.time, report.person_id, report.infection_status
    );
}

fn print_report_header() {
    println!("time,person_id,infection_status");
}

pub fn init(context: &mut Context) {
    print_report_header();
    context.subscribe_to_event::<InfectionStatusEvent>(|context, event| {
        handle_infection_status_change(context, event);
    });
}
