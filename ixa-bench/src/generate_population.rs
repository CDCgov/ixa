use clap::{Arg, Command};
use csv::Writer;
use rand::seq::SliceRandom;
use rand::Rng;

const MIN_AGE: u8 = 0;
const MAX_AGE: u8 = 100;
const SCHOOL_AGE_MIN: u8 = 5;
const SCHOOL_AGE_MAX: u8 = 18;
const WORK_AGE_MIN: u8 = 18;
const WORK_AGE_MAX: u8 = 65;
const HOUSEHOLD_SIZE: usize = 2;

#[derive(Debug)]
pub struct Person {
    pub id: usize,
    pub age: u8,
    pub home_id: usize,
    pub school_id: usize,
    pub workplace_id: usize,
}

#[derive(Debug)]
pub struct Population {
    pub people: Vec<Person>,
    pub number_of_homes: usize,
    pub number_of_schools: usize,
    pub number_of_workplaces: usize,
}

#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_precision_loss)]
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::must_use_candidate)]
pub fn generate_population(
    n: usize,
    number_of_schools_as_percent_of_pop: f64,
    number_of_workplaces_as_percent_of_pop: f64,
) -> Population {
    let num_schools = ((n as f64 * number_of_schools_as_percent_of_pop / 100.0).round()) as usize;
    let num_workplaces =
        ((n as f64 * number_of_workplaces_as_percent_of_pop / 100.0).round()) as usize;
    let num_homes = usize::max(1, n / HOUSEHOLD_SIZE);
    let school_ids: Vec<usize> = (1..=num_schools).collect();
    let workplace_ids: Vec<usize> = (1..=num_workplaces).collect();
    let home_ids: Vec<usize> = (1..=num_homes).collect();
    let mut rng = rand::thread_rng();
    let mut people = Vec::with_capacity(n);

    for i in 0..n {
        let age = rng.gen_range(MIN_AGE..=MAX_AGE);
        let home_id = *home_ids.choose(&mut rng).unwrap();
        let mut school_id = 0;
        let mut workplace_id = 0;
        if (SCHOOL_AGE_MIN..=SCHOOL_AGE_MAX).contains(&age) {
            school_id = *school_ids.choose(&mut rng).unwrap();
        }
        if (WORK_AGE_MIN..=WORK_AGE_MAX).contains(&age) {
            workplace_id = *workplace_ids.choose(&mut rng).unwrap();
        }
        people.push(Person {
            id: i + 1,
            age,
            home_id,
            school_id,
            workplace_id,
        });
    }
    Population {
        people,
        number_of_homes: num_homes,
        number_of_schools: num_schools,
        number_of_workplaces: num_workplaces,
    }
}

#[allow(dead_code)]
fn save_population_to_csv(population: &Population, output_file: &str) {
    let mut wtr = Writer::from_path(output_file).expect("Cannot create output file");
    wtr.write_record(["id", "age", "homeId", "schoolId", "workplaceId"])
        .unwrap();
    for person in &population.people {
        wtr.write_record(&[
            person.id.to_string(),
            person.age.to_string(),
            person.home_id.to_string(),
            person.school_id.to_string(),
            person.workplace_id.to_string(),
        ])
        .unwrap();
    }
    wtr.flush().unwrap();
}

// cargo run --release -- 1000 --schools_percent 0.2 --workplaces_percent 10 --output random_population.csv
#[allow(dead_code)]
fn main() {
    let matches = Command::new("Generate random population CSV file")
        .arg(
            Arg::new("n")
                .help("Size of population")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("schools_percent")
                .long("schools_percent")
                .help("Number of schools as percent of population")
                .num_args(1)
                .default_value("0.2"),
        )
        .arg(
            Arg::new("workplaces_percent")
                .long("workplaces_percent")
                .help("Number of workplaces as percent of population")
                .num_args(1)
                .default_value("10"),
        )
        .arg(
            Arg::new("output")
                .long("output")
                .help("Output CSV file name")
                .num_args(1)
                .default_value("random_population.csv"),
        )
        .get_matches();

    let n = matches
        .get_one::<String>("n")
        .unwrap()
        .parse::<usize>()
        .expect("n must be an integer");
    let schools_percent = matches
        .get_one::<String>("schools_percent")
        .unwrap()
        .parse::<f64>()
        .expect("schools_percent must be a float");
    let workplaces_percent = matches
        .get_one::<String>("workplaces_percent")
        .unwrap()
        .parse::<f64>()
        .expect("workplaces_percent must be a float");
    let output = matches.get_one::<String>("output").unwrap();

    let population = generate_population(n, schools_percent, workplaces_percent);
    println!("Number of people: {}", population.people.len());
    println!("Number of homes: {}", population.number_of_homes);
    println!("Number of schools: {}", population.number_of_schools);
    println!("Number of workplaces: {}", population.number_of_workplaces);
    save_population_to_csv(&population, output);
}
