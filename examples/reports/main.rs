use ixa::context::Context;

fn main() {
    let mut context = Context::new();

    #[cfg(feature = "reports")]
    context.add_report::<Incidence>("incidence");
    #[cfg(feature = "reports")]
    context.add_report::<Death>("death");

    context.add_plan(1.0, |context| {
        #[cfg(feature = "reports")]
        context.send_report(Incidence {
            person_id: 1,
            t: context.get_current_time(),
        });
        println!(
            "Person 1 was infected at time {}",
            context.get_current_time()
        );
    });

    context.add_plan(2.0, |context| {
        #[cfg(feature = "reports")]
        context.send_report(Death {
            person_id: 1,
            t: context.get_current_time(),
        });
        println!("Person 1 died at time {}", context.get_current_time());
    });

    context.execute();
}
