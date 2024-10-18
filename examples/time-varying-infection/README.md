# Infection model: time-varying force of infection and recovery
This example is centered around demonstrating two of the classical
ways of modeling a time-varying force in a continuous time simulation.
The two approaches are inverse transform sampling and rejection sampling,
and I explain what both of these approaches are and when to use each of them.
I explore the use of inverse transform sampling to deal with a force
of infection that changes based on the season and rejection sampling to
deal with a recovery rate that depends on the number of infected people.

The goal of this example is two-fold:
1. Explain the math and process behind properly modeling a time-varying rate.
    - Develop best practices for when model math should be done in `ixa` vs
    the pre-processing stages to best drive modularity.
2. Provide a template from which we can bench-test complicated examples of
time-varying infection to explore the need for us to establish `ixa`
standards for modeling time-varying rates.
3. Bonus: the basic-infection example did not use `ixa`'s internal  `people`
or `global_properties` modules, so this example also updates to using the
built-in functionality.

Some aspects worth thinking about standardizing on time-varying rates:
- Format of time-varying input data: as a function, or random samples from
the distribution? If a function, how should it be discretized?
- Library of curves needed to improve rejection sampling efficiency
- What Rust tools should be used when modeling time-varying rates?
- When should integration be done within Rust vs externally and fed in
as a model input? If a model input, how should the data be formatted?

Conceptually, the structure in this model is same as the `basic-infection`
example except with time-varying forces of infection/recovery. As such, this
readme instead focuses on explaining why modeling time-varying infection in
continuous time requires a bit of math and then why inverse transform sampling
and rejection sampling are two viable approaches and their associated pros/cons.

There is one big caveat of this model. Because we are trying to avoid
person-to-person transmission for now, I've made the force of infection
seasonal and the recovery rate dependent on the number of infected people.
If the force of infection depended on the number of infected people, we'd
basically have an SIR model. But, in reality, the technique I describe with
dealing for the time-varying foi is much more likely to be applied to a
time-varying recovery rate (depends on, say, both time since infection
and time of the simulation aka seasonality) while the technique I describe
for dealing with a time-varying recovery rate may be more likely applied to
force of infection in real-world models.

## Why are time-varying rates special?
The classic SIR compartmental ODE model assumes a constant recovery rate,
$\gamma$, so that the infection period is exponentially distributed. In
the real-world, infection periods are often not exponentially distributed.
However, observational studies often provide a distribution for the actual
infection period. One might use that distribution as an input to their ABM
and then draw from that empirical distribution when scheduling an
individual's recovery. This is an entirely valid approach.

However, it is generally easier to work in terms of rates rather than
distributions. In other words, it is useful to have the recovery rate
over time. For any distribution besides the exponential distribution,
the recovery rate is time-varying. Having a rate is useful because (a)
there is a direct analogue to a corresponding compartmental model, and (b)
clinical studies often report rates for people based on some
demographic factor in a proportional hazards model, and then the user
can assemble the individual's overall rate from their
demographic characteristics and the demography-->corresponding rate
values provided by the clinical study.

This leads us to the following question: given a rate function, $f(t)$,
how can we accurately draw times of recovery or infection that follow the
given rate over time? We can't just draw values from an exponential distribution
anymore -- what would we put as the rate? You can probably see that if we
know $f(t)$ as the hazard rate, we know the corresponding PDF, so we could
draw recovery times from that distribution. This is the crux of the idea
behind inverse-transform sampling. You may probably also see that if
you drew times at some constant rate faster than the rate function, you could
evaluate whether the recovery event had happened at a given time even
without knowing how the recovery rate function may change in the future.
This is the crux of the idea behind rejection sampling.

Below, I go into greater detail about each of these two methods and their tradeoffs.
The ultimate goal of this example is to demonstrate how modeling time-varying rates
is really just a simple extension over time-constant rates within `ixa` and how
we can use the ideas presented here to build up towards a time-varying infection
rates when the infection is person-to-person rather than just environmental.

## Time-varying force of infection
In a constant force of infection model (like the classic compartmental ODE SIR),
a person experiences a constant hazard rate of infection. Therfore, we
use the exponential distribution to plan for the time at which a person
will fall sick. With time-varying infection, the hazard rate is not constant
over time, so we must draw times from an alternate distribution, one that
follows the time-varying hazard rate and therefore properly apportions people's
sickness events at the right times.

### What is inverse-transform sampling?
If $\textrm{foi}(t)$ describes the hazard rate for infection at a given time,
$1 - e ^ {-\int_0^t \textrm{foi}(u)du}$ is the cumulative probability of
infection by $t$ elapsed. In the case where $\textrm{foi}(t) = k$, we recover an
exponential distribution. This is just stating the relationship between the
hazard rate and the CDF, but this identity provides us with what we need
to go from the hazard rate to the distribution function. Now we must figure
out how to draw random values of $t$ that follow the given CDF.

Since the integral is a CDF, if we draw a random number $u \sim \mathcal{U}(0, 1)$,
set $u$ equal to the CDF, and solve for the corresponding value of $t$,
we obtain a random sample of $t$ that follows the arbitrary distribution
defined by the hazard rate. This is a generic strategy that works because
the CDF is a transformation that takes any distribution and turns it into a
uniform distribution.

In other words, we have a method of going from $\mathcal{U}(0, 1)$ to
samples of $t$. We can do one additional step of math to make the work
needed to be done by the modeler easier. In general, the CDF of an
exponentially distributed random variable with rate 1, $s \sim \textrm{Exp}(1)$,
is $F(s) = 1 - e^{-s}$. We can rewrite the integral of $\textrm{foi}(t)$ instead as
$F(\int_0^t \textrm{foi}(u)du)$. As such, we see that to obtain a uniform
distributed random variable on (0, 1), we have to pass the integral of the foi
through an exponential CDF. We can bypass this step if we instead draw an
exponential random variable, $s$, and set that equal to
$\int_0^t \textrm{foi}(u)du$, we have a slight shortcut for generating
samples of $t$ that does not require taking a natural logarithm. Overall,
this technique of expressing an abstract distribution in terms of samples
of another distribution is called inverse-transform sampling because
it exploits inverting the CDF of the abstract distribution to be able to
express it in terms of some other distribution.

In our particular example of food-borne illness, based on the function
$\textrm{foi}(t)$, we can pre-schedule everyone's infection at the beginning
of the simulation and infections will occur at the correct nonuniformly
distributed rate. This is because we have an environmental disease, so
eventually all people will get infected. There is an implicit difference
in how this ABM is set up versus the last example: rather than drawing infection
_attempt_ events and _then_ picking a person to infect (as was done in the last
example), we schedule infection _transition_ events for all people at the beginning
of the simulation and then just execute their transitions at the given time.
This is a bit more of the _individual_-specific approach. The other benefit
of this approach in this particular example is that there is no longer a need
for a `MAX_TIME` after which infection attempts cannot be scheduled: instead,
the simulation will just end once everyone has been infected (pre-scheduled
at simulation beginning) and they all recover.

### Implementation

Let's look at some pseudo-code. In this pseudo example, we pick
$\textrm{foi}(t) = \sin(t + c) + 1$ where $c$ is a user parameter.

```rust
use roots::find_root_brent;
use reikna::integral::integrate;
define_rng!(InfectionRng);

pub enum DiseaseStatus {
    S, I, R
}

define_person_property_with_default!(DiseaseStatusType,
                                     DiseaseStatus,
                                     DiseaseStatus::S);

define_person_property_with_default!(InfectionTime, Option<f64>, None);

fn init(context: &mut Context) {
    // let deviled eggs be our food borne illness
    // as soon as a person enters the simulation, they are exposed to deviled eggs
    // based on foi(t), they will have their infection planned at a given time
    context.subscribe_to_event(move |context, event: PersonCreatedEvent| {
        expose_person_to_deviled_eggs(context, event);
    });
}

fn expose_person_to_deviled_eggs(context: &mut Context,
                                 person_created_event: PersonCreatedEvent) {
    // when the person is exposed to deviled eggs, make a plan for them to fall
    // sick based on foi(t), where inverse sampling is used to draw times from
    // the corresponding distribution
    inverse_sampling_infection(context, person_created_event.person_id());
}

// parameterize the foi
fn foi(t: f64, sin_shift: f64) -> f64 {
    f64::sin(t + sin_shift) + 1 // foi must always be greater than 1
}

fn inverse_sampling_infection(context: &mut Context, person_id: PersonID) {
    // random exponential value
    let s: f64 = context.sample_distr(InfectionRng, Exp1);
    // get the time by following the formula described above
    // first need to get the simulation's sin_shift
    let parameters = context.get_global_property_value(Parameters).clone();
    let sin_shift = parameters.foi_sin_shift;
    let f = func!(move |t| foi(t, sin_shift));
    // as easy as Python to integrate and find roots in Rust!
    let f_int_shifted = move |t| integrate(&f, 0, t) - s;
    let t = find_root_brent(0f64, 100f64, // lower and upper bounds for the root finding
                            f_int_shifted).unwrap();
    context.add_plan(t, move |context| {
        context.set_person_property(person_id, DiseaseStatus, DiseaseStatusType::I);
        // for reasons that will become apparent with the recovery rate example,
        // we also need to record the time at which a person becomes infected
        context.set_person_property(person_id, InfectionTime, t);
    });
}

```

### Caveats

There are some constraints of vanilla inverse-transform sampling.
1. We needed to be able to write down the way the force of infection varies with
time as a hazard function (or, more generally, any type of distribution function).
It is possible that we know, from data, the mean waiting time of illness and standard
deviation. In that case, some approximation will need to be made for the hazard
function. This issue speaks to a more general problem of incorporating real-world
data into ABMs.
2. We needed to know the function $\textrm{foi}(t)$ a priori. Imagine a model
where the time-varying rate depends on some internal state of the model, so the
modeler does not know how the time-varying rate will change over time as it
can only be determined as the model is running. Then, this approach will not work.
Instead, rejection sampling provides a strategy for taking draws from an arbitrary
and potentially changing distribution, and more is discussed on this below.
    - Similarly, in this model, we needed to pre-assign all the infection transitions.
    Had we not done that, we would have had to use rejection sampling instead
    because we would be evaluating whether an infection had happened by a given time
    or not as we proceeded through time. However, scheduling the infection attempts
    (similarly to in the last example) rather than transitions has a benefit
    of only infecting the existing people in the simulation (or, region/partition)
    at the time of event rather than pre-scheduling the transition and needing to
    cancel it if the person dies or is somehow no longer eligible for infection.
3. Sampling a new value of $t$ requires inverting an integral function
(i.e., $\int_0^t \textrm{foi}(u)du$). Not only must this process be done every time
a new sample is required, but inverting the function may not be straightforward. This
is potentially computationally inefficient and makes inverse-transform sampling prine
to the errors associated with inverting any function numerically.

## Time-varying recovery rate

Imagine that the recovery rate scales inversely with the number of infected
people (so that the recovery time increases with the number of infected people).
A potential biological explanation of this would be that infected people
require some medicine, but their time to getting that medicine depends on how
many other people are infected.

This scenario has a key difference from the preceeding example: at the
point of someone's infection, we cannot schedule their recovery because we do
not know exactly what the number of infected people over time will be
out into the future. (OK, just kidding. We kinda actually do because this example
is a deterministic model, but calculating the number of infecteds at every time
in the future while accounting for recovery sounds like  a pain, so please continue
playing pretend with me that we do not know.) In other words, we can't analytically
write out a priori $\textrm{for}(q)$ (force of recovery over time time since
infection, $q$). So, we have to use an alternative sampling technique to obtain
draws from the time-varying recovery rate distribution.

### What is rejection sampling?

Rejection sampling is similarly grounded in understanding the CDF. The CDF tells
us the probability that recovery has happened at some time since infection.
Imagine obtaining a probability that recovery has happened at some time, $t_j$
from the CDF, $p_j$, and using a Bernoulli distribution with parameter $p_j$
to assess whether the recovery event has happened. If you obtain a Bernoulli
sample of 1, you know that recovery has happened by that time. Now, imagine that
you had just sampled some value of time $q_i < q_j$. If you had obtained a
Bernoulli sample of 0 at $q_i$ and then obtain a 1 at $q_j$, you would know
that this person's recovery must have happened between the two times.
For sufficiently small $q_2 - q_1$, it is fair to say that the recovery
event happens at $q_2$.

In other words, by assessing whether recovery has happened by making sequential
samples from a Bernoulli distribution with probability parameter obtained
from the CDF, one can obtain samples of the underlying distribution for which
we have written the CDF.

However, there's a big catch -- we need to do some work to figure out the
subsequent times we should be checking the CDF, in other words finding the
value of $q_j - q_i$ or the time between sequential checks from the CDF to
assess whether recovery has happened. Imagine a trivial case where we pick a
value of $q_j - q_i = \tau(t)$ that is infinitely small. We would be having
events in our model at every $\tau(t)$. That would be painfully inefficient.
On the other hand, if we pick some large $\tau(t)$, we may skip over when the
recovery event really happens and schedule it to happen later than it should,
biasing the sequence of events.

So, what is the biggest $\tau(t)$ (slowest rate of events) that we can have
that would still enable our simulation to be accurate? First, note how I have
written $\tau$ to be a function of time. This rate can change over time. Let's
develop some intuition for what it should be.

First, let us write down the CDF of recovery, similar to how we did
this with rejection sampling. If we say that recovery rate scales
inversely with the number of infected people, we can write the following:
$\textrm{CDF}_\textrm{recovery}(q, t) = 1 - \exp(-q/n(t))$ where $n(t)$ is
the effective number of infected people (effective because it may be
scaled by a disease recovery rate).

The reason we don't know the recovery time a priori is becaues the recovery
rate changes with the number of infected people. Imagine the case where the
recovery rate does not change with the number of people. Then, we would just
evaluate whether recovery has happened at some $1/\gamma$ (recovery rate) period.
However, because there is another process by which the number of infected people
changes the recovery rate, we must figure out the maximum rate at which
infected people can change -- that is 2 in this toy model (it is 2 because
the foi is equal to a sin function plus 1). So, recovery must be evaluated
at the fastest of these two rates -- meaning that the resampling rate
is their sum, $2 + 1 / \gamma$.

### Implementation

```rust
define_rng!(RecoveryRng);

fn init(context: &mut Context) {
    context.subscribe_to_event(move |context,
                               event: PersonPropertyChangeEvent<DiseaseStatusType>| {
        handle_infection_status_change(context, event);
    });
}

fn handle_infection_status_change(context: &mut Context,
                                  event: PersonPropertyChangeEvent<DiseaseStatusType>) {
    let parameters = context.get_global_property_value(Parameters).clone();
    if matches!(event.current, DiseaseStatus::I) {
        evaluate_recovery(context, event.person_id, parameters.foi * 2.0 + 1.0 / parameters.infection_distribution);
    }
}

fn recovery_cdf(context: &mut Context, time_spent_infected: f64) -> f64 {
    1 - f64::exp(-time_spent_infected * n_effective_infected(context))
}

fn n_effective_infected(context: &mut Context) -> f64 {
    let parameters = context.get_global_property_value(Parameters).clone();
    // get number of infected people
    let mut n_infected = 0;
    for usize_id in 0..context.get_current_population() {
        if matches!(context.get_person_property(context.get_person_id(usize_id),
                                       DiseaseStatusType), DiseaseStatus::I) {
                                        n_infected = n_infected + 1;
                                       }
    }
    parameters.gamma / n_infected
}

fn evaluate_recovery(context: &mut Context, person_id: PersonId, resampling_rate: f64) {
    // get time person has spent infected
    let time_spent_infected = context.get_current_time() - context.get_person_property(person_id,
    InfectionTime)
    // evaluate whether recovery has happened by this time or not
    let recovery_probability = recovery_cdf(context, time_spent_infected);
    if context.sample_bool(RecoveryRng, recovery_probability) {
        // recovery has happened by now
        context.set_person_property(person_id, DiseaseStatusType, DiseaseStatus::R);
    } else {
        // add plan for recovery evaluation to happen again at fastest rate
        context.add_plan(context.get_current_time() + context.sample_distr(ExposureRng,
        Exp::new(resampling_rate).unwrap()),
        move |context| {
        evaluate_recovery(context, person_id, resampling_rate);
    });
    }
}
```

### Caveats

1. Rejection sampling is inherently inefficient. It is effectively
a guess and check method and requires scheduling events just to evaluate
whether the real event in question has happened or not. As such, it
requires scheduling more events than are transitions that actually
happen in the simulation -- there are ancillary events that have no
impact on simulation state. There are ways around this problem --
for instance, instead of always resampling at the maximal rate of change,
one could resample at the local maximal rate of change. In fact,
one could make a series of linear functions that approximate a given
unknown distribution that is being modeled. Development of such a
functional library is a valuable area of `ixa` advancement. This
becomes increasingly important if the distribution from which we are
trying to sample has a high peak -- then, the maximal value at which
we sample must be based on that high peak, which is inefficient for the
majority of the distribution.

2. Rejection sampling is effectively a geometric sampling technique.
In essence, we sampling in two dimensions (time values at which we sample
and whether the sampled value is a viable draw from the distribution) to
obtain samples from a one dimensional distribution. As such, if we were to
expand rejection sampling from a distribution in more dimensions, it is natural
to see that the method will always be subject to the curse of dimensionality
and sampling will become increasingly inefficient.

## Sidenotes

1. Imagine that we had to take multiple draws from a time-varying
rate's distribution -- for instance, scheduling infection
attempts based on the generation distribution. We would have to take
all of our samples from the generation distribution at the moment a person
is infected and then have infection attempts happen at the ordering
of the times. This becomes messy, and explaining how to do this
is the subject of a future example.

2. Note that MCMC can be used to obtain correlated samples from paired
distributions.
