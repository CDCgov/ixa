use ixa_example_basic_infection::initialize;

fn main() {
    let mut context = initialize().expect("Error adding report.");
    context.execute();
}
