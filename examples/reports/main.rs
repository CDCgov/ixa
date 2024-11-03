use ixa::context::Context;
use ixa::error::IxaError;
use ixa::report::ContextReportExt;
use ixa::{create_report_trait, report::Report};
use serde::{Deserialize, Serialize};

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

fn init() -> Result<Context, IxaError> {
    let mut context = Context::new();
    context.add_report::<Incidence>("incidence.csv")?;
    context.add_report::<Death>("death.csv")?;

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

    Ok(context)
}

fn main() {
    let init_result = init();
    match init_result {
        Ok(mut context) => {
            context.execute();
            println!("Simulation completed successfully");
        }
        Err(ixa_error) => {
            println!("Initialization failure: {ixa_error}");
        }
    }
}
