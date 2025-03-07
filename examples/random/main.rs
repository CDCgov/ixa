use ixa::context::Context;
use ixa::define_rng;
use ixa::random::ContextRandomExt;
use rand::distributions::Uniform;
use rand::Rng;

static SEED: u64 = 123;
static POPULATION: u64 = 10;

define_rng!(MyRng);

fn main() {
    let mut context = Context::new();
    context.init_random(SEED);

    let random_person = context.sample_range(MyRng, 0..POPULATION);

    context.add_plan(1.0, {
        move |context| {
            println!(
                "Person {} was infected at time {}",
                random_person,
                context.get_current_time()
            );
        }
    });

    let recovery_time = if context.sample_bool(MyRng, 0.5) {
        context.sample_distr(MyRng, Uniform::new(2.0, 10.0))
    } else {
        context.sample(MyRng, |rng| rng.gen_range(10.0..20.0))
    };

    context.add_plan(recovery_time, {
        move |context| {
            println!(
                "Person {random_person} recovered at time {}",
                context.get_current_time()
            );
        }
    });
    context.execute();
}
