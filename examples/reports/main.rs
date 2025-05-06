use ixa::prelude::*;
use ixa::report::serialize_f64;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
struct Incidence {
    person_id: String,
    #[serde(serialize_with = "serialize_f64::<_,2>")]
    t: f64,
}

#[derive(Serialize, Deserialize, Clone)]
struct Death {
    person_id: String,
    #[serde(serialize_with = "serialize_f64::<_,2>")]
    t: f64,
}

create_report_trait!(Incidence);
create_report_trait!(Death);

#[allow(unexpected_cfgs)]
fn initialize() -> Result<Context, IxaError> {
    let mut context = Context::new();

    context
        .report_options()
        .file_prefix("Reports_".to_string())
        .directory(PathBuf::from("./"))
        .overwrite(true); // Not recommended for production. See `basic-infection/incidence-report`.;
    context.add_report::<Incidence>("incidence")?;
    context.add_report::<Death>("death")?;
    Ok(context)
}

fn main() {
    let mut context = initialize().expect("Error adding report.");

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
