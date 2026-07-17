use std::time::Duration;

use ixa::profiling::{
    add_computed_statistic, increment_named_count, open_span, ProfilingContextExt,
};
use ixa::runner::run_with_args;
use ixa::{define_entity, define_property, with, ContextEntitiesExt};

define_entity!(ProfilingPerson);
define_property!(struct ProfilingAge(u8), ProfilingPerson);
define_property!(struct ProfilingCounty(u8), ProfilingPerson);

fn main() {
    let mut context = run_with_args(|context, _args, _| {
        context.add_plan(0.0, |context| {
            context
                .add_entity(with!(ProfilingPerson, ProfilingAge(42), ProfilingCounty(1)))
                .unwrap();
            context
                .add_entity(with!(ProfilingPerson, ProfilingAge(7), ProfilingCounty(2)))
                .unwrap();
            assert_eq!(
                context
                    .query_result_iterator(with!(ProfilingPerson, ProfilingAge(42)))
                    .count(),
                1
            );
            assert_eq!(
                context
                    .query_result_iterator(with!(
                        ProfilingPerson,
                        ProfilingAge(42),
                        ProfilingCounty(1)
                    ))
                    .count(),
                1
            );
            assert_eq!(
                context
                    .query_result_iterator(with!(
                        ProfilingPerson,
                        ProfilingCounty(1),
                        ProfilingAge(42)
                    ))
                    .count(),
                1
            );
            assert_eq!(
                context
                    .query_result_iterator(with!(
                        ProfilingPerson,
                        ProfilingAge(7),
                        ProfilingCounty(2)
                    ))
                    .count(),
                1
            );

            increment_named_count("it_prof_event");
            increment_named_count("it_prof_event");
            increment_named_count("it_prof_event");

            {
                let _span = open_span("it_prof_span");
                // Ensure elapsed runtime is nonzero so derived rates/percentages are well-defined.
                std::thread::sleep(Duration::from_millis(10));
            }

            add_computed_statistic::<usize>(
                "it_prof_stat",
                "Total test events",
                Box::new(|data| data.counts.get("it_prof_event").copied()),
                Box::new(|_value| {}),
            );

            context.shutdown();
        });
        Ok(())
    })
    .unwrap();

    context.write_profiling_data();
}
