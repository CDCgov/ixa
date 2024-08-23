use ixa::context::Context;
use ixa::report::ContextReportExt;
use ixa::{create_report_trait, report::Report};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
struct Incidence {
    person_id: String,
    t: f64,
}

#[derive(Serialize, Deserialize, Clone)]
struct Death {
    person_id: String,
    t: f64,
}

create_report_trait!(Incidence);
create_report_trait!(Death);

#[allow(unexpected_cfgs)]
fn main() {
    let mut context = Context::new();

    context
        .report_options()
        .file_prefix("Reports_".to_string())
        .directory(PathBuf::from("./"));
    context.add_report::<Incidence>("incidence");
    context.add_report::<Death>("death");

    context.add_plan(1.0, |context| {
        context.send_report(Incidence {
            person_id: 1.to_string(),
            t: context.get_current_time(),
        });
        println!(
            "Person 1 was infected at time {}",
            context.get_current_time()
        );
    });

    context.add_plan(2.0, |context| {
        context.send_report(Death {
            person_id: 1.to_string(),
            t: context.get_current_time(),
        });
        println!("Person 1 died at time {}", context.get_current_time());
    });

    context.execute();
}
