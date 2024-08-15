# Infection model: constant force of infection
The purpose of this example is to model the infection process in a homogeneous population assuming a constant force of infection, such as from the environment.

## Entities, states and variables

- individuals: represent people in a population who can have three states representing their infection status: susceptible, infected, or recovered.
  - susceptible: individuals with this status can be infected
  - infected: represents active infection. In this model, infected individuals are not able to transmit to others. After a recovery period, infected individuals update their status to recovered.
  - recovered: represents recovery from infection. Individuals with this state are not able to get infected.
- variables:
  - population size: number of individuals to include in the simulation
  - force of infection: rate at which susceptible individuals become infected
  - infection period: time that an individual spends from infection to recovery

## Simulation overview
The first infection attempt is scheduled at time 0. Infection events are scheduled to occur based on the constant force of infection. Once an infection event is scheduled, a susceptible individual is selected to be infected.  After infection attempt is finished, the next infection event is scheduled based on the constant force of infection. The simulation ends after no more infection events are scheduled.

Infected individuals schedule their recovery at time `t + infected period`. The infection status of recovered individuals remains as recovered for the rest of the simulation.

### Infection state transitions
```mermaid
flowchart LR
S(Susceptible) --FoI--> I(Infected) --inf. period--> R(Recovered)
```
## Simulation architecture
The simulation executaion first initializes the model parameters and prepares the simulation modules.

### Initialization and modules

 - *parameters module*: this module manages data for parameter values for population size, force of infection, and infection period. At initialization, all parameter variables are set to a specific value.
 - *person module*: contains a unique ID for each person that identifies each person in the simulation.
 - *`person_infection_status` module*: this module connects each person ID with a specific value for a person's infection status, which could be one of Susceptible, Infected, or Recovered.
 - *population manager module*: The population manager module is in charge of setting up the population. At initialization, the module reads the population size (N) parameter and creates N persons with unique ids (1..N) and sets their initial infection status to Susceptible.
 - *transmission module*: This module is in charge of spreading the infection through the population. At initialization, it schedules an infection attempt for time 0. An infection attempt consists of choosing at random a susceptible individual from the population and setting their infection status to **infected**. Each infection attempt, also schedules the next infection attempt, which time is based on the force of infection.
 - *infection manager module*: This module controls the infection status of each person. Transmission of pathogen is not handled in this module, only individual's infection status. Specifically, it handles the progression of newly infected persons by scheduling recovery, which is drawn from an exponential distribution based on the infection period parameter from the parameters module.
 - *reports module*: This module reports changes in infection status.
 - *random_number_generator module*: Two random number generators are required. One to control the sequence of susceptible persons drawn in the transmission module, and another to control the specific times of recovery in the infection manager module.

### Control flow
#### Trasmission manager
- Module data: This modules doesn't hold any specific data
- Dependencies:
```
    context, parameters, random number generator, person infection status, person, population manager
```
- Initialization:
```
    init(context) {
        context.add_rng(id = transmission);
        context.add_plan(attempt_infection(context), time = 0);
    }

```
- Methods:
```
    attempt_infection(context) {
        transmission_rng = rng.get_rng(id = transmission);
        population = context.get_population();
        person_to_infect = transmission_rng.sample_int(from = 0, to = population);

        if (context.get_infection_status(person_to_infect) == Susceptible) {
            context.set_infection_status(person_to_infect, Infected);
        }

        foi = parameters.get_parameter(foi);
        time_next_infection = transmission_rng.draw_exponential(1/foi);
        context.add_plan(attempt_infection(context), time = context.get_time() + time_next_infection);
    }
```

#### Infection manager
- Module data: No specific data for this module.
- Dependencies: random number generator, person infection status, person, context, parameters.
- Initialization: Adds a new random number generator and subscribes to events generated by changes in infection status.
```
    init(context) {
        context.add_rng(id = infection);

        // This function in context should send, time, person_id, and infection status change.
        context.observe_infection_status_event(handle_infection_status_change);
    }
```
- Methods:
```
    handle_infection_status_change(context, person_id, old_infection_status) {
        if (context.get_infection_status(person_id) == Infected) {
            infection_rng = context.get_rng(id = infection);
            infection_period = parameters.get_parameter(infection_period)
            recovery_time = infection_rng.draw_exponential(1/infection_period);
            context.add_plan(context.set_infection_status(person_id, Recovered), time = recovery_time);
        }
    }
```

### Events and observation
- Changes in the infection status of individuals release an event that are observed by the simulation context.
- Only events changing from susceptible to infected are handled by scheduling a change in infection status to recovered based on the infected period.

## Stochasticity
Stochasticity in the simulation affects
 - the timing of infections,
 - the order of individuals  infected, and
 - times to recovery from infected individuals.
