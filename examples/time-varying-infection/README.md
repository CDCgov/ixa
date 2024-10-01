# Infection model: time-varying force of infection and recovery
This example is centered around demonstrating two of the classical
ways of modeling a time-varying force in a continuous time simulation.
I explore the use of inverse transform sampling to deal with a force
of infection that is sinusoidal and rejection sampling to deal with a
recovery rate that depends on the number of infected people.

The goal of this example is two-fold:
1) Explain the math and process behind properly modeling a time-varying force.
2) Demonstrate how to implement this math in Rust and within the `ixa` framework.
    - Develop best practices for when model math should be done in `ixa` vs
    the pre-processing stages to best drive modularity.
3) Bonus: Bench-test complicated example of time-varying infection to show that
they are simple to implement with `ixa`'s functionality.

Conceptually, the structure/flow in this model is same as the `basic-infection`
example except with time-varying forces of infection/recovery. As such, this
readme focuses on explaining why modeling time-varying infection in continuous
time requires a bit of math and then why inverse transform sampling and rejection
sampling are two viable approaches and their associated pros/cons.

## Time-varying force of infection
In a constant force of infection model, a person experiences a constant hazard
rate of infection. Therfore, we use the exponential distribution to plan for
the time at which a person will fall sick. With time-varying infection, the hazard
rate is not constant over time, so we must draw times from an alternate distribution,
one that follows the time-varying hazard rate and therefore properly apportions people's
sickness events at the right times.

### What is inverse-transform sampling?

If foi$(t)$ describes the hazard rate for infection at a given time,
$1 - e ^ {-\int_0^t \textrm{foi}(u)du}$ is the cumulative probability of
infection at $t$. Note that this is a CDF, so if we draw a random number
$u \sim \mathcal{U}(0, 1)$, set $u$ equal to the CDF, and solve for the
corresponding value of $t$, we obtain a random sample of $t$ that follows
the arbitrary distribution defined by the hazard rate. This is a generic
strategy that works because the CDF is a transformation that takes any
distribution and turns it into a uniform distribution.

In other words, we have a method of going from $\mathcal{U}(0, 1)$ to
samples of $t$. We can do one additional step of math to make the work
needed to be done by the modeler easier. In general, the CDF of an
exponentially distributed random variable with rate 1, $s ~ \textrm{Exp}(1)$,
is $F(s) = 1 - e^{-s}$. As such, if we instead draw an exponential
random variable, $s$, and set that equal to $\int_0^t \textrm{foi}(u)du$,
we have a slight shortcut for generating samples of $t$ that does not
require a natural logarithm.

In our particular example of food-borne illness, based on the function $\textrm{foi}(t)$,
we can pre-schedule everyone's infection at the beginning of the simulation and infections
will occur at the correct nonuniformly distributed rate. Note that there is an implicit
difference in how this ABM is set up versus the last example: rather than drawing infection
_attempt_ events and _then_ picking a person to infect, we schedule infection _transition_
events for all people at the beginning of the simulation (recall, we have an environmental
disease so eventually all people will get infected). This is a bit more of the _individual_-specific
approach.

### Implementation

Let's look at some pseudo-code. In this pseudo example, we pick
$\textrm{foi}(t) = \sin(t + c) + 1$ where $c$ is a user parameter.

```rust
use roots::find_root_brent;
use reikna::integral::*;
define_rng!(InfectionRng);

fn init(context: &mut Context) {
    context.subscribe_to_event(PersonCreationEvent, expose_person_to_deviled_eggs);
}

fn expose_person_to_deviled_eggs(context: &mut Context,
                                 person_creation_event: PersonCreationEvent) {
    inverse_sampling_infection(context, person_creation_event.person_id());
}

// parameterize the foi
fn foi(t: f64, sin_shift: f64) -> f64 {
    f64::sin(t + sin_shift) + 1 // foi must always be greater than 1
}

fn inverse_sampling_infection(context: &mut Context, person_id: PersonID) {
    // random exponential value
    let s = context.sample_distr(InfectionRng, Exp1::new());
    // get the time by following the formula described above
    // first need to get the simulation's sin_shift
    let sin_shift = parameters.get_parameter(foi_sin_shift);
    let f = func!(move |t| foi(t, sin_shift));
    // as easy as Python to integrate and find roots in Rust!
    let f_int_shifted = func!(move |t| integrate(&f, 0, t) - s);
    let t = find_root_brent(0f64, 100f64, // guesses for the root bracketing
                            f_int_shifted).unwrap();
    context.add_plan(context.set_person_property(person_id, Properties::infection_time, context.get_time()),
                        t)
}

```

### Caveats

However, there are some constraints of vanilla inverse-transform sampling.
1) We needed to be able to write down the way the force of infection varies with
time as a hazard function (or, more generally, any type of distribution function).
It is possible that we know, from data, the mean waiting time of illness and standard
deviation. In that case, some approximation will need to be made for the distribution
function. This issue speaks to a more general problem of incorporating real-world
data into ABMs.
2) We needed to know the function $\textrm{foi}(t)$ a priori. Imagine a model
where the time-varying rate depends on some internal state of the model, so the
modeler does not know how the time-varying rate will change over time as it
can only be determined as the model is running. Then, this approach will not work.
Instead, rejection sampling provides a strategy for taking draws from an arbitrary
and potentially changing distribution, and more is discussed on this below.
3) Sampling a new value of $t$ requires inverting an integral function
(i.e., $\int_0^t \textrm{foi}(u)du$). Not only must this process be done every time
a new sample is required, but inverting the function may not be straightforward. This
is potentially computationally inefficient and prone to the errors associated with inverting
any function numerically.

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
is a deterministic model, but calculating the number of infecteds while accounting
for recovery sounds like  a pain, so please continue playing pretend with me that
we do not know.) In other words, we can't analytically write out a priori $\textrm{for}(q)$
(force of recovery over time time since infection, $q$). So, we have to use an
alternative sampling technique to obtain draws from the time-varying recovery rate
distribution.

### What is rejection sampling?

Rejection sampling is similarly grounded in understanding the CDF. The CDF tells
us the probability that recovery has happened at some time since infection.
Imagine obtaining a probability that recovery has happened at some time, $t_2$
from the CDF, $p_2$, and using a Bernoulli distribution to assess whether
the recovery event has happened. If you obtain a Bernoulli sample of 1, you
know that recovery has happened by that time. Now, imagine that you had just
sampled some value of time $q_1 < q_2$. If you had obtained a Bernoulli sample of 0
at $q_1$, you would know that this person's recovery must have happened between the
two times. For sufficiently small $q_2 - q_1$, it is fair to say that the recovery
event happens at $q_2$.

In other words, by assessing whether recovery has happened by making sequential
samples from a Bernoulli distribution with probability parameter obtained
from the CDF, one can obtain samples of the underlying distribution for which
we have written the CDF.

However, there's a big catch -- finding the value of $q_2 - q_1$ or the time
between sequential checks from the CDF to assess whether recovery has happened.
Imagine a trivial case where we pick a value of $q_2 - q_1 = \tau(t)$ that is infinitely
small. We would be having events in our model at every $\tau(t)$. That would be
painfully inefficient. So, what is the biggest $\tau(t)$ that we can have
that would still enable our simulation to be accurate? First, note how I have written
$\tau$ to be a function of time. This rate can change over time. In fact, it really just
must be the maximal possible rate of change in the recovery rate for a given time.

Let's develop some intuition for this:

There's also another matter I've brushed over: writing down the CDF. In this case,
that is actually quite simple. If we say that recovery rate scales inversely
with the number of infected people, we can write the following:

$\textrm{CDF}(q, t) = 1 - \exp(-q*n(t))$ where $n(t)$ is the effective
number of infected people (effective because it may be scaled by a recovery rate).

## Alternative ways of modeling time-varying forces
