# Example - Ixa - Simple transmission model

Infection process in a homogeneous closed population with infection process independently from contact process and disease progression.

- Specifications

  - Time-varying infectiousness
  - Disease progression: symptom, including hospitalizations.

- Contact patterns
  - Homogeneous but contacts should be reduced when hospitalized
  - Closed population

- Outputs
  - Disease and infection status: incidence and in periodic reports that represent the number of people on each status.

## Motivation
This example shows how to implement a simple transmission model where symptoms and infection are treated separately. This model requires some basic characteristics from Ixa:

  - person properties: infection and symptom status are specified in person properties.
  - random number generator: transmission requires the evaluation and generation of random numbers.
  - input/output handling
  - basic reports: periodic report where reports are scheduled to output prevalence values of person properties, incidence report for disease status where changes in disease status are outputed.
  - contact manager: a way to keep track of contacts for each person.
  - plans: plans are necessary to schedule events related to infection progression, symptom progression, transmission, and reports.

## Model description
This model reproduces transmission dynamics of a pathogen in a closed homogeneous population. Individuals are modeled as perons with two properties related to their infection and health status. Infection status specifies the course of infection for an individual exposed to the pathogen. An individual is susceptible to infection. When a susceptible individual is exposed to the pathogen through contact with an infectious person, the susceptible individual is considered infected with the pathogen but unable to infect others. After the latent period, the infected individual becomes infectious and is able to infect others through contact. The number of new infections is defined in this example by the basic reproductive number.

Infected individuals may develop symptoms after exposure based on the incubation period. Those who develop symptoms could require hospitalization, which is defined in the model by a probability of hospitalization, time from symptom onset to hospitalization, and hospitalization duration. During hospitalization, the contacts from an individual should be restricted. In this simple example, and because this is a homogeneous population, we assume that hospitalization effectively reduces all contacts to zero.


## How to run the model
To run the model:

`cargo build -r`

Run a single example scenario:

`target/release/cfa-eosim-facemask -i test/input/config.yaml -o test/output/`

Run multiple example scenarios with 4 threads:

`target/release/cfa-eosim-facemask -i test/input/config_multi.yaml -o test/output/ -t 4`

Inspect the output in R using the script `test/output/plot_output.R`
