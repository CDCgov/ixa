//! Verifies the hierarchical network example in `docs/book/src/topics/networks.md`.

use ixa::prelude::*;

define_entity!(County);
define_entity!(Person);

// Which county a person lives in. Indexed, so "everyone in county c" is one lookup.
define_property!(struct Home(CountyId), Person);

// Coarse level: counties that exchange travellers. Edge data is the share of a
// resident's contacts that happen in the neighbouring county.
define_network!(Travel, County, data = f64);

// Fine level: who lives with whom.
define_network!(Household, Person);

define_rng!(ContactRng);

const HOUSEHOLD_SHARE: f64 = 0.5;

fn sample_contact(context: &Context, infector: PersonId) -> Option<PersonId> {
    if context.sample_bool(ContactRng, HOUSEHOLD_SHARE) {
        if let Some(edge) = context.sample_edge(ContactRng, Household, infector) {
            return Some(edge.neighbor);
        }
    }

    // Otherwise mix within a county: travel along an edge with probability equal
    // to the total outgoing share, else stay home.
    let Home(home) = context.get_property(infector);
    let away: f64 = context
        .get_edges(Travel, home)
        .iter()
        .map(|edge| edge.data)
        .sum();
    let county = if context.sample_bool(ContactRng, away) {
        context
            .sample_edge_weighted(ContactRng, Travel, home, |edge| edge.data)?
            .neighbor
    } else {
        home
    };

    context
        .sample_entity(ContactRng, with!(Person, Home(county)))
        .filter(|&contact| contact != infector)
}

fn build() -> (Context, PersonId, Vec<CountyId>) {
    let mut context = Context::new();
    context.init_random(7);
    context.index_property::<Person, Home>();

    let counties: Vec<CountyId> = (0..3)
        .map(|_| context.add_entity(with!(County)).unwrap())
        .collect();
    context
        .add_edge_bidi(Travel, counties[0], counties[1], 0.2)
        .unwrap();
    context
        .add_edge_bidi(Travel, counties[1], counties[2], 0.1)
        .unwrap();

    let mut first = None;
    for &county in &counties {
        for _ in 0..10 {
            let a = context.add_entity(with!(Person, Home(county))).unwrap();
            let b = context.add_entity(with!(Person, Home(county))).unwrap();
            context.add_edge_bidi(Household, a, b, ()).unwrap();
            first.get_or_insert(a);
        }
    }
    (context, first.unwrap(), counties)
}

#[test]
fn contacts_come_from_household_home_or_neighbouring_county() {
    let (context, infector, counties) = build();
    let Home(home) = context.get_property(infector);
    assert_eq!(home, counties[0]);
    let housemate = context.get_edges(Household, infector)[0].neighbor;

    let mut household = 0;
    let mut same_county = 0;
    let mut neighbour = 0;
    for _ in 0..2000 {
        // A county draw can land on the infector; a model would treat that as no contact.
        let Some(contact) = sample_contact(&context, infector) else {
            continue;
        };
        let Home(county) = context.get_property(contact);
        if contact == housemate {
            household += 1;
        } else if county == home {
            same_county += 1;
        } else {
            assert_eq!(county, counties[1], "county 2 is not adjacent to county 0");
            neighbour += 1;
        }
    }
    // Expected of 2000: ~1040 household (0.5 direct plus the housemate drawn from the
    // county), ~720 elsewhere in the home county, ~200 in the neighbouring county, ~40 self.
    assert!((940..=1140).contains(&household), "{household}");
    assert!((650..=850).contains(&same_county), "{same_county}");
    assert!((150..=250).contains(&neighbour), "{neighbour}");
}
