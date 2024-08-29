use ixa::context::Context;
use ixa::define_rng;
use ixa::random::ContextRandomExt;
use rand::distributions::Uniform;

static SEED: u64 = 123;
static POPULATION: u64 = 10;

define_rng!(MyRng);

fn main() {
    let mut context = Context::new();
    context.init_random(SEED);

    let random_person = context.sample_range(MyRng, 0..POPULATION);
    let person_id = random_person;

    context.add_plan(1.0, {
        move |context| {
            println!(
                "Person {} was infected at time {}",
                person_id,
                context.get_current_time()
            );
        }
    });

    let recovery_time: f64 = context.sample_distr(MyRng, Uniform::new(2.0, 10.0));
    context.add_plan(1.0, {
        move |_context| {
            println!("Person {person_id} recovered at time {recovery_time}");
        }
    });
    context.execute();
}
