use std::{fs::File, path::Path};

use cfa_eosim_facemask::sir::{
    death_manager::DeathManager,
    global_properties::{
        DeathRate, HospitalizationDelay, HospitalizationDuration, IncubationPeriod,
        InfectiousPeriod, InitialInfections, LatentPeriod, MaxDays, Population,
        ProbabilityHospitalized, ProbabilitySymptoms, SymptomaticPeriod, R0,
    },
    infection_manager::InfectionManager,
    infection_seeder::InfectionSeeder,
    periodic_report::{PeriodicReport, PeriodicStatus},
    person_property_report::{PersonPropertyChange, PersonPropertyReport},
    population_loader::PopulationLoader,
    transmission_manager::TransmissionManager,
};
use clap::Parser;
use eosim::{
    context::Context,
    global_properties::GlobalPropertyContext,
    random::RandomContext,
    reports::{get_channel_report_handler, Report, ReportsContext},
};
use serde_derive::{Deserialize, Serialize};
use threadpool::ThreadPool;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, Sender};

#[derive(Debug, Parser)]
struct SirArgs {
    /// Input config file
    #[arg(short, long)]
    input: String,
    /// Output directory
    #[arg(short, long)]
    output: String,
    /// Number of threads
    #[arg(short, long, default_value_t = 1)]
    threads: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
struct Parameters {
    population: usize,
    r0: f64,
    infectious_period: f64,
    latent_period: f64,
    incubation_period: f64,
    max_days: usize,
    probability_symptoms: f64,
    symptomatic_period: f64,
    hospitalization_duration: f64,
    probability_hospitalized: f64,
    hospitalization_delay: f64,
    initial_infections: usize,
    random_seed: u64,
    death_rate: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
struct Scenario {
    scenario: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Config {
    Single(Parameters),
    Multiple(Vec<Parameters>),
}

fn setup_context(context: &mut Context, parameters: &Parameters) {
    // Set up parameters in simulation
    context.set_global_property_value::<Population>(parameters.population);
    context.set_global_property_value::<R0>(parameters.r0);
    context.set_global_property_value::<InfectiousPeriod>(parameters.infectious_period);
    context.set_global_property_value::<LatentPeriod>(parameters.latent_period);
    context.set_global_property_value::<IncubationPeriod>(parameters.incubation_period);
    context.set_global_property_value::<SymptomaticPeriod>(parameters.symptomatic_period);
    context
        .set_global_property_value::<HospitalizationDuration>(parameters.hospitalization_duration);
    context.set_global_property_value::<HospitalizationDelay>(parameters.hospitalization_delay);
    context
        .set_global_property_value::<ProbabilityHospitalized>(parameters.probability_hospitalized);
    context.set_global_property_value::<ProbabilitySymptoms>(parameters.probability_symptoms);
    context.set_global_property_value::<InitialInfections>(parameters.initial_infections);
    context.set_global_property_value::<DeathRate>(parameters.death_rate);
    context.set_global_property_value::<MaxDays>(parameters.max_days);

    // Set up RNG
    context.set_base_random_seed(parameters.random_seed);

    // Add reports
    context.add_component::<PersonPropertyReport>();
    context.add_component::<PeriodicReport>();

    // Add model components
    context.add_component::<PopulationLoader>();
    context.add_component::<InfectionManager>();
    context.add_component::<TransmissionManager>();
    context.add_component::<InfectionSeeder>();
    context.add_component::<DeathManager>();
}

pub fn get_bounded_channel_report_handler<T: Report, S>(
    sender: Sender<(S, T::Item)>,
    id: S,
) -> impl FnMut(T::Item) + 'static
where
    T::Item: serde::Serialize + Send + 'static,
    S: serde::Serialize + Send + Copy + 'static,
{
    move |item| {
        let sender = sender.clone();
        let id = id;
        futures::executor::block_on(async move {
            if let Err(e) = sender.send((id, item)).await {
                panic!("Receiver being closed, failed to send item: {:?}", e);
            }
        });
    }
}

fn run_single_threaded(parameters_vec: Vec<Parameters>, output_path: &Path) {
    let output_file = File::create(output_path.join("person_property_report.csv"))
        .expect("Could not create output file.");
    let output_periodic_file = File::create(output_path.join("periodic_report.csv"))
        .expect("Could not create output periodic file.");
    for (scenario, parameters) in parameters_vec.iter().enumerate() {
        let mut writer_builder = csv::WriterBuilder::new();
        // Don't re-write the headers
        if scenario > 0 {
            writer_builder.has_headers(false);
        }
        let mut writer = writer_builder.from_writer(
            output_file
                .try_clone()
                .expect("Could not write to output file"),
        );
        let mut periodic_writer = writer_builder.from_writer(
            output_periodic_file
                .try_clone()
                .expect("could not write to output file for periodic report"),
        );
        // Set up and execute context
        let mut context = Context::new();
        context.set_report_item_handler::<PersonPropertyReport>(move |item| {
            if let Err(e) = writer.serialize((Scenario { scenario }, item)) {
                eprintln!("{}", e);
            }
        });
        context.set_report_item_handler::<PeriodicReport>(move |item| {
            if let Err(e) = periodic_writer.serialize((Scenario { scenario }, item)) {
                eprintln!("{}", e);
            }
        });
        setup_context(&mut context, parameters);
        context.execute();
        println!("Scenario {} completed", scenario);
    }
}

async fn run_multi_threaded(parameters_vec: Vec<Parameters>, output_path: &Path, threads: u8) {
    let output_file = File::create(output_path.join("person_property_report.csv"))
        .expect("Could not create output file.");
    let output_periodic_file = File::create(output_path.join("periodic_report.csv"))
        .expect("Could not create output periodic file.");
    let pool = ThreadPool::new(threads.into());
    let (sender, mut receiver) = mpsc::channel::<(Scenario, PersonPropertyChange)>(100000);
    let (periodic_sender, mut periodic_receiver) =
        mpsc::channel::<(Scenario, PeriodicStatus)>(100000);

    let handle = Handle::current();

    for (scenario, parameters) in parameters_vec.iter().enumerate() {
        let sender = sender.clone();
        let periodic_sender = periodic_sender.clone();
        let parameters = *parameters;
        let handle = handle.clone();
        pool.execute(move || {
            let _guard = handle.enter();
            // Set up and execute context
            let mut context = Context::new();
            context.set_report_item_handler::<PersonPropertyReport>(
                get_bounded_channel_report_handler::<PersonPropertyReport, Scenario>(
                    sender,
                    Scenario { scenario },
                ),
            );
            context.set_report_item_handler::<PeriodicReport>(
                get_bounded_channel_report_handler::<PeriodicReport, Scenario>(
                    periodic_sender,
                    Scenario { scenario },
                ),
            );
            setup_context(&mut context, &parameters);
            context.execute();
            println!("Scenario {} completed", scenario);
        });
    }
    drop(sender);
    drop(periodic_sender);

    // Write output from main thread
    let mut person_property_writer = csv::Writer::from_writer(output_file);
    let mut periodic_writer = csv::Writer::from_writer(output_periodic_file);
    loop {
        tokio::select! {
            Some(item) = receiver.recv() => {
                person_property_writer.serialize(item).unwrap();
            },
            Some(item) = periodic_receiver.recv() => {
                periodic_writer.serialize(item).unwrap();
            },
            else => break,
        }
    }
}

#[tokio::main]
async fn main() {
    // Parse args and load parameters
    let args = SirArgs::parse();
    let config_file = File::open(&args.input)
        .unwrap_or_else(|_| panic!("Could not open config file: {}", args.input));
    let config: Config = serde_yaml::from_reader(config_file).expect("Could not parse config file");
    let output_path = Path::new(&args.output);

    match config {
        Config::Single(parameters) => run_single_threaded(vec![parameters], output_path),
        Config::Multiple(parameters_vec) => {
            if args.threads <= 1 {
                run_single_threaded(parameters_vec, output_path)
            } else {
                run_multi_threaded(parameters_vec, output_path, args.threads).await;
            }
        }
    }
}
