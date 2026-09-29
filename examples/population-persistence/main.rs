use std::path::PathBuf;

use ixa::IxaError;

mod gen_population;
mod model;
mod people;

fn main() -> Result<(), IxaError> {
    let population_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("population-persistence")
        .join("output")
        .join("population.bin");

    gen_population::generate(&population_path)?;
    let summary = model::run(&population_path)?;
    println!(
        "Loaded {} people ({} initially infected, total age {}).",
        summary.people, summary.infected, summary.total_age
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_population_round_trips_into_model() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("population.bin");

        gen_population::generate(&path).unwrap();
        assert_eq!(
            model::run(&path).unwrap(),
            model::PopulationSummary {
                people: 3,
                infected: 1,
                total_age: 109,
            }
        );
    }
}
