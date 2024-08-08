# Infection model: constant force of infection 
The purpose of this example is to model the infection process in a homogeneous population assuming a constant force of infection, such as from the environment. 

## Entities, states and variables

- individuals: represent people in a population who can have three states representing their infection status: susceptible, infected, or recovered. 
  - susceptible: individuals with this status can be infected 
  - infected: represents active infection. In this model, infected individuals are not able to transmit to others. After a recovery period, infected individuals update their status to recovered. 
  - recovered: represents recovery from infection. Individuals with this state are not able to get infected. 
- variables:
  - force of infection: rate at which susceptible individuals become infected
  - infected period: time that an individual spends from infection to recovery

## Process overview 
```{mermaid}
flowchart LR
A(Susceptible) -> B(Infected) -> C(Recovered)
```
