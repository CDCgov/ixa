use ixa_example_births_deaths::initialize;

fn main() {
    let mut context = initialize().expect("Could not initialize context.");
    context.execute();
}
