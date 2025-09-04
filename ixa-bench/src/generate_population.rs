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

pub struct PopulationIterator {
    n: usize,
    idx: usize,
    num_schools: usize,
    num_workplaces: usize,
    num_homes: usize,
    rng: rand::rngs::ThreadRng,
}

impl PopulationIterator {
    pub fn new(
        n: usize,
        number_of_schools_as_percent_of_pop: f64,
        number_of_workplaces_as_percent_of_pop: f64,
    ) -> Self {
        let num_schools =
            ((n as f64 * number_of_schools_as_percent_of_pop / 100.0).round()) as usize;
        let num_workplaces =
            ((n as f64 * number_of_workplaces_as_percent_of_pop / 100.0).round()) as usize;
        let num_homes = usize::max(1, n / HOUSEHOLD_SIZE);
        let rng = rand::thread_rng();
        PopulationIterator {
            n,
            idx: 0,
            num_schools,
            num_workplaces,
            num_homes,
            rng,
        }
    }
}

impl Iterator for PopulationIterator {
    type Item = Person;
    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.n {
            return None;
        }
        let age = self.rng.gen_range(MIN_AGE..=MAX_AGE);
        let home_id = self.rng.gen_range(1..=self.num_homes);
        let mut school_id = 0;
        let mut workplace_id = 0;
        if (SCHOOL_AGE_MIN..=SCHOOL_AGE_MAX).contains(&age) && self.num_schools > 0 {
            school_id = self.rng.gen_range(1..=self.num_schools);
        }
        if (WORK_AGE_MIN..=WORK_AGE_MAX).contains(&age) && self.num_workplaces > 0 {
            workplace_id = self.rng.gen_range(1..=self.num_workplaces);
        }
        let person = Person {
            id: self.idx + 1,
            age,
            home_id,
            school_id,
            workplace_id,
        };
        self.idx += 1;
        Some(person)
    }
}

pub fn generate_population(
    n: usize,
    number_of_schools_as_percent_of_pop: f64,
    number_of_workplaces_as_percent_of_pop: f64,
) -> PopulationIterator {
    PopulationIterator::new(
        n,
        number_of_schools_as_percent_of_pop,
        number_of_workplaces_as_percent_of_pop,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_population_stats() {
        let n = 1000;
        let schools_percent = 0.2;
        let workplaces_percent = 10.0;
        let num_homes = usize::max(1, n / HOUSEHOLD_SIZE);
        let num_schools = ((n as f64 * schools_percent / 100.0).round()) as usize;
        let num_workplaces = ((n as f64 * workplaces_percent / 100.0).round()) as usize;

        let population_iter = generate_population(n, schools_percent, workplaces_percent);
        let people: Vec<Person> = population_iter.collect();

        assert_eq!(people.len(), n);
        // Check that home_id, school_id, workplace_id are in valid ranges
        for person in &people {
            assert!(person.age >= MIN_AGE && person.age <= MAX_AGE);
            assert!(person.home_id >= 1 && person.home_id <= num_homes);
            if person.school_id != 0 {
                assert!(person.school_id >= 1 && person.school_id <= num_schools);
            }
            if person.workplace_id != 0 {
                assert!(person.workplace_id >= 1 && person.workplace_id <= num_workplaces);
            }
        }
    }
}
