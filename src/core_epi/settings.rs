//! Settings: named kinds of place (household, school, ...) whose instances
//! group people, with a density exponent per kind and an itinerary per person
//! saying how their time is split across kinds.
//!
//! Define the kinds with [`define_settings!`](crate::define_settings), then at
//! setup register each kind's `alpha`, create groups and add members (by
//! external key when loading a line list, see
//! [`setting_group_for_key`](SettingsExt::setting_group_for_key); or make
//! a kind a population group everyone is in), and assign itineraries: one
//! [`Itinerary`] for everyone, one per value of a property, or one computed
//! per person ([`ItineraryAssignment`]), with
//! [`set_itinerary`](SettingsExt::set_itinerary) overriding for individuals.
//!
//! The multiplier on intrinsic infectiousness in a group of `n` people is
//! `(n - 1)^alpha`: with `alpha = 1` attempts scale with group size, with
//! `alpha = 0` they do not. A person's current multiplier is the itinerary
//! weighted average over their groups (the itinerary renormalized over the
//! kinds they have a group in); their bound is the largest multiplier
//! any of those groups has ever had, so members can be removed and added back
//! (interventions) without re-forecasting. In
//! [`TransmissionManager`](super::TransmissionManager), `forecast_total_infectiousness`
//! scales the intrinsic bound by
//! [`max_setting_multiplier`](SettingsExt::max_setting_multiplier), and
//! `evaluate_forecast` draws the setting first with
//! [`sample_setting`](SettingsExt::sample_setting) (by itinerary ratio over
//! the kinds the person has a group in), thins by that setting's
//! [`setting_multiplier`](SettingsExt::setting_multiplier), and samples a
//! contact there with
//! [`sample_setting_contact`](SettingsExt::sample_setting_contact). Drawing by
//! ratio and thinning by one multiplier gives attempts in each setting at the
//! same rate as thinning by the ratio weighted average and then drawing by
//! `ratio * multiplier`, with one group lookup per proposal instead of one
//! per kind.
//!
//! Per attempt that is a store lookup, the person's membership records (one
//! per kind, person-major so they share a cache line) for the draw, one group
//! record for the multiplier (cached per group) and the contact, and two
//! uniform draws. Nothing is hashed or allocated; itineraries are small `Copy`
//! values.

use std::cell::Cell;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

use smallvec::SmallVec;

use crate::core_epi::person::{Person, PersonId};
use crate::data_structures::entity_map::EntityMap;
use crate::entity::property::Property;
use crate::hashing::one_shot_128;
use crate::rand::{Rng, RngExt};
use crate::random::sample_single_excluding;
use crate::{define_rng, Context, ContextBase, ContextEntitiesExt, ContextRandomExt, HashMap};

define_rng!(SettingsRng);

/// The most kinds one enum may have.
pub const MAX_SETTINGS: usize = 8;

/// A kind of setting, as an enum from [`define_settings!`](crate::define_settings).
pub trait SettingKind: Copy + Eq + Debug + 'static {
    const ALL: &'static [Self];

    /// Position in [`ALL`](Self::ALL).
    fn index(self) -> usize;

    fn name(self) -> &'static str;

    #[doc(hidden)]
    fn store<C: ContextBase>(context: &C) -> &SettingsStore<Self>;

    #[doc(hidden)]
    fn store_mut<C: ContextBase>(context: &mut C) -> &mut SettingsStore<Self>;
}

/// Defines an enum of setting kinds and the storage behind it.
///
/// ```ignore
/// define_settings!(pub Setting { Household, Community });
/// ```
#[macro_export]
macro_rules! define_settings {
    ($vis:vis $name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        $vis enum $name {
            $($variant),+
        }

        $crate::paste::paste! {
            $crate::define_data_plugin!(
                [<$name SettingsPlugin>],
                $crate::core_epi::settings::SettingsStore<$name>,
                $crate::core_epi::settings::SettingsStore::new()
            );

            impl $crate::core_epi::settings::SettingKind for $name {
                const ALL: &'static [Self] = &[$(Self::$variant),+];

                fn index(self) -> usize {
                    self as usize
                }

                fn name(self) -> &'static str {
                    match self {
                        $(Self::$variant => stringify!($variant)),+
                    }
                }

                fn store<C: $crate::ContextBase>(
                    context: &C,
                ) -> &$crate::core_epi::settings::SettingsStore<Self> {
                    context.get_data([<$name SettingsPlugin>])
                }

                fn store_mut<C: $crate::ContextBase>(
                    context: &mut C,
                ) -> &mut $crate::core_epi::settings::SettingsStore<Self> {
                    context.get_data_mut([<$name SettingsPlugin>])
                }
            }
        }
    };
}

/// One instance of a setting kind: a particular household, school, ...
pub struct SettingGroupId<S> {
    index: u32,
    kind: PhantomData<S>,
}

impl<S> SettingGroupId<S> {
    fn new(index: u32) -> Self {
        Self {
            index,
            kind: PhantomData,
        }
    }
}

impl<S> Clone for SettingGroupId<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S> Copy for SettingGroupId<S> {}
impl<S> PartialEq for SettingGroupId<S> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}
impl<S> Eq for SettingGroupId<S> {}
impl<S> Debug for SettingGroupId<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SettingGroupId({})", self.index)
    }
}

/// How a person's time is split across setting kinds. Proportions are
/// normalized, so they need not sum to one. A small `Copy` value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Itinerary<S> {
    ratios: [f64; MAX_SETTINGS],
    kind: PhantomData<S>,
}

impl<S: SettingKind> Itinerary<S> {
    /// # Panics
    ///
    /// If a proportion is negative or not finite, or none is positive.
    pub fn new(entries: impl IntoIterator<Item = (S, f64)>) -> Self {
        let mut ratios = [0.0; MAX_SETTINGS];
        for (setting, proportion) in entries {
            assert!(
                proportion.is_finite() && proportion >= 0.0,
                "proportion for setting {} must be finite and non-negative, got {proportion}",
                setting.name()
            );
            ratios[setting.index()] += proportion;
        }
        let total: f64 = ratios.iter().sum();
        assert!(
            total > 0.0,
            "itinerary needs at least one positive proportion"
        );
        for ratio in &mut ratios {
            *ratio /= total;
        }
        Self {
            ratios,
            kind: PhantomData,
        }
    }

    /// The normalized proportion of time in `setting`.
    pub fn ratio(&self, setting: S) -> f64 {
        self.ratios[setting.index()]
    }

    /// Every kind with its normalized proportion.
    pub fn entries(&self) -> impl Iterator<Item = (S, f64)> + '_ {
        S::ALL.iter().map(move |&s| (s, self.ratios[s.index()]))
    }
}

type IndividualFn<S> = Box<dyn Fn(&Context, PersonId) -> Itinerary<S>>;

/// Where people's itineraries come from: one for everyone, one per value of a
/// property, or one computed per person. Resolved on every lookup.
pub struct ItineraryAssignment<S>(Assign<S>);

enum Assign<S> {
    Global(Itinerary<S>),
    Individual(IndividualFn<S>),
}

impl<S: SettingKind> ItineraryAssignment<S> {
    /// The same itinerary for everyone.
    pub fn global(itinerary: Itinerary<S>) -> Self {
        Self(Assign::Global(itinerary))
    }

    /// An itinerary per value of property `P`, from the person's current value.
    pub fn by_property<P, F>(f: F) -> Self
    where
        P: Property<Person>,
        F: Fn(P) -> Itinerary<S> + 'static,
    {
        Self::individual(move |context, person| f(context.get_property(person)))
    }

    /// An itinerary per person.
    pub fn individual<F>(f: F) -> Self
    where
        F: Fn(&Context, PersonId) -> Itinerary<S> + 'static,
    {
        Self(Assign::Individual(Box::new(f)))
    }

    fn resolve(&self, context: &Context, person: PersonId) -> Itinerary<S> {
        match &self.0 {
            Assign::Global(itinerary) => *itinerary,
            Assign::Individual(f) => f(context, person),
        }
    }
}

/// Anything [`set_itinerary_assignment`](SettingsExt::set_itinerary_assignment)
/// accepts: an [`ItineraryAssignment`], a bare [`Itinerary`] (global), or a
/// closure `Fn(&Context, PersonId) -> Itinerary<S>` (individual).
pub trait IntoItineraryAssignment<S> {
    fn into_itinerary_assignment(self) -> ItineraryAssignment<S>;
}

impl<S: SettingKind> IntoItineraryAssignment<S> for ItineraryAssignment<S> {
    fn into_itinerary_assignment(self) -> ItineraryAssignment<S> {
        self
    }
}

impl<S: SettingKind> IntoItineraryAssignment<S> for Itinerary<S> {
    fn into_itinerary_assignment(self) -> ItineraryAssignment<S> {
        ItineraryAssignment::global(self)
    }
}

impl<S, F> IntoItineraryAssignment<S> for F
where
    S: SettingKind,
    F: Fn(&Context, PersonId) -> Itinerary<S> + 'static,
{
    fn into_itinerary_assignment(self) -> ItineraryAssignment<S> {
        ItineraryAssignment::individual(self)
    }
}

/// `(n - 1)^alpha` for a group of `n`, 0 for an empty one. The common
/// exponents skip the `powf`.
fn multiplier(alpha: f64, n: usize) -> f64 {
    if n == 0 {
        return 0.0;
    }
    let others = (n - 1) as f64;
    if alpha == 0.0 {
        1.0
    } else if alpha == 1.0 {
        others
    } else {
        others.powf(alpha)
    }
}

/// Position of a member who has left their group but remembers it.
const NOT_ACTIVE: u32 = u32::MAX;
/// Group of a person who never had one of that kind.
const NO_GROUP: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct Member {
    group: u32,
    position: u32,
}

impl Member {
    const NONE: Self = Self {
        group: NO_GROUP,
        position: NOT_ACTIVE,
    };
}

struct Kind {
    alpha: Option<f64>,
    /// A group everyone is in without an entry in `memberships`.
    population_group: Option<u32>,
    /// Groups created by external key, e.g. a line list's household ids.
    keys: HashMap<u128, u32>,
}

struct Group {
    kind: usize,
    /// Member indices, inline for the small groups most households are.
    members: SmallVec<[u32; 4]>,
    /// The most members the group has ever had.
    max_size: usize,
    /// `(n - 1)^alpha` for the current and the largest ever size, computed on
    /// the first read after a membership change so that bulk loading pays no
    /// `powf` per member.
    multipliers: Cell<Option<(f64, f64)>>,
}

impl Group {
    fn resized(&mut self) {
        self.max_size = self.max_size.max(self.members.len());
        self.multipliers.set(None);
    }

    fn multipliers(&self, alpha: f64) -> (f64, f64) {
        self.multipliers.get().unwrap_or_else(|| {
            let m = (
                multiplier(alpha, self.members.len()),
                multiplier(alpha, self.max_size),
            );
            self.multipliers.set(Some(m));
            m
        })
    }
}

/// Storage for one setting kind enum; defined per enum by
/// [`define_settings!`](crate::define_settings).
pub struct SettingsStore<S> {
    kinds: Vec<Kind>,
    /// One `Member` per person per kind, person-major with stride
    /// `kinds.len()`, so an attempt's lookups across kinds share a cache line.
    memberships: Vec<Member>,
    groups: Vec<Group>,
    assignment: Option<ItineraryAssignment<S>>,
    /// Per-person overrides of the assignment.
    itineraries: EntityMap<Person, Itinerary<S>>,
}

impl<S: SettingKind> Default for SettingsStore<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: SettingKind> SettingsStore<S> {
    #[doc(hidden)]
    #[must_use]
    pub fn new() -> Self {
        assert!(
            S::ALL.len() <= MAX_SETTINGS,
            "at most {MAX_SETTINGS} setting kinds are supported"
        );
        Self {
            kinds: S::ALL
                .iter()
                .map(|_| Kind {
                    alpha: None,
                    population_group: None,
                    keys: HashMap::default(),
                })
                .collect(),
            memberships: Vec::new(),
            groups: Vec::new(),
            assignment: None,
            itineraries: EntityMap::new(),
        }
    }

    fn alpha(&self, setting: S) -> f64 {
        self.kinds[setting.index()]
            .alpha
            .unwrap_or_else(|| panic!("setting {} is not registered", setting.name()))
    }

    /// `person`'s membership record of `setting`; `Member::NONE` if they never
    /// had a group of it.
    fn member(&self, person: PersonId, setting: S) -> Member {
        self.memberships
            .get(person.0 * self.kinds.len() + setting.index())
            .copied()
            .unwrap_or(Member::NONE)
    }

    fn member_mut(&mut self, person: PersonId, setting: S) -> &mut Member {
        let stride = self.kinds.len();
        let index = person.0 * stride + setting.index();
        if index >= self.memberships.len() {
            self.memberships
                .resize((person.0 + 1) * stride, Member::NONE);
        }
        &mut self.memberships[index]
    }

    /// The group `person` is in now, if any.
    fn group_of(&self, person: PersonId, setting: S) -> Option<u32> {
        let m = self.member(person, setting);
        if m.group == NO_GROUP {
            self.kinds[setting.index()].population_group
        } else {
            (m.position != NOT_ACTIVE).then_some(m.group)
        }
    }

    /// The group `person` is in or was removed from, if any.
    fn remembered_group_of(&self, person: PersonId, setting: S) -> Option<u32> {
        let m = self.member(person, setting);
        if m.group == NO_GROUP {
            self.kinds[setting.index()].population_group
        } else {
            Some(m.group)
        }
    }

    /// `person`'s current and largest-ever multiplier in `setting`, given the
    /// population size for a population group; `None` without a group.
    fn multipliers(&self, person: PersonId, setting: S, population: usize) -> Option<(f64, f64)> {
        let group = self.group_of(person, setting)?;
        Some(self.group_multipliers(group, setting, population))
    }

    fn group_multipliers(&self, group: u32, setting: S, population: usize) -> (f64, f64) {
        if self.kinds[setting.index()].population_group == Some(group) {
            let m = multiplier(self.alpha(setting), population);
            (m, m)
        } else {
            self.groups[group as usize].multipliers(self.alpha(setting))
        }
    }

    fn itinerary(&self, context: &Context, person: PersonId) -> Option<Itinerary<S>> {
        self.itineraries
            .get(person)
            .copied()
            .or_else(|| Some(self.assignment.as_ref()?.resolve(context, person)))
    }

    /// Take `person` out of their current list group of `setting`, if any,
    /// remembering the group.
    fn remove(&mut self, person: PersonId, setting: S) {
        let member = self.member(person, setting);
        if member.group == NO_GROUP || member.position == NOT_ACTIVE {
            return;
        }
        self.member_mut(person, setting).position = NOT_ACTIVE;
        let group = &mut self.groups[member.group as usize];
        group.members.swap_remove(member.position as usize);
        let moved = group.members.get(member.position as usize).copied();
        group.resized();
        if let Some(moved) = moved {
            self.member_mut(PersonId::new(moved as usize), setting)
                .position = member.position;
        }
    }

    fn sample_contact<R: Rng>(
        &self,
        context: &Context,
        rng: &mut R,
        person: PersonId,
        setting: S,
    ) -> Option<PersonId> {
        let group = self.group_of(person, setting)?;
        if self.kinds[setting.index()].population_group == Some(group) {
            context.query(Person).sample_entity_excluding(rng, person)
        } else {
            sample_single_excluding(rng, &self.groups[group as usize].members, person.0 as u32)
                .map(|&i| PersonId::new(i as usize))
        }
    }
}

/// The kind a uniform draw `u` in `[0, 1)` lands on, in proportion to
/// `weights`; `None` if all are 0.
fn draw_setting<S: SettingKind>(weights: &[f64; MAX_SETTINGS], u: f64) -> Option<S> {
    let total: f64 = weights.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let mut acc = 0.0;
    let mut last = None;
    for &setting in S::ALL {
        let w = weights[setting.index()];
        if w > 0.0 {
            acc += w;
            last = Some(setting);
            if u * total < acc {
                return last;
            }
        }
    }
    last
}

/// Setting membership, itineraries and the quantities the transmission loop
/// needs from them. `S` is inferred from the arguments.
pub trait SettingsExt: ContextBase + ContextRandomExt + ContextEntitiesExt {
    /// Register `setting` with density exponent `alpha`. Call once per kind at
    /// setup, before creating its groups.
    fn register_setting<S: SettingKind>(&mut self, setting: S, alpha: f64) {
        assert!(
            alpha.is_finite() && alpha >= 0.0,
            "alpha for setting {} must be finite and non-negative, got {alpha}",
            setting.name()
        );
        S::store_mut(self).kinds[setting.index()].alpha = Some(alpha);
    }

    /// Create an empty group of `setting`.
    fn create_setting_group<S: SettingKind>(&mut self, setting: S) -> SettingGroupId<S> {
        let store = S::store_mut(self);
        store.alpha(setting);
        store.groups.push(Group {
            kind: setting.index(),
            members: SmallVec::new(),
            max_size: 0,
            multipliers: Cell::new(None),
        });
        SettingGroupId::new(u32::try_from(store.groups.len() - 1).expect("too many setting groups"))
    }

    /// The group of `setting` with external `key`, created on first use: for
    /// loading a line list whose rows carry setting ids. Use one key type per
    /// kind; keys are compared by hash.
    ///
    /// ```
    /// # use ixa::core_epi::{Person, SettingsExt};
    /// # use ixa::prelude::*;
    /// # ixa::define_settings!(Setting { Household });
    /// # let mut context = Context::new();
    /// # context.register_setting(Setting::Household, 1.0);
    /// let line_list = "person_id,household_id\n1,HH001\n2,HH001\n3,HH002\n";
    /// let mut people = Vec::new();
    /// for row in csv::Reader::from_reader(line_list.as_bytes()).records() {
    ///     let row = row.unwrap();
    ///     let person = context.add_entity(Person).unwrap();
    ///     let household = context.setting_group_for_key(Setting::Household, &row[1]);
    ///     context.add_setting_member(household, person);
    ///     people.push(person);
    /// }
    /// assert_eq!(context.setting_group_size(people[0], Setting::Household), 2);
    /// assert_eq!(context.setting_group_size(people[2], Setting::Household), 1);
    /// ```
    fn setting_group_for_key<S: SettingKind>(
        &mut self,
        setting: S,
        key: impl Hash,
    ) -> SettingGroupId<S> {
        let hash = one_shot_128(&key);
        if let Some(&index) = S::store(self).kinds[setting.index()].keys.get(&hash) {
            return SettingGroupId::new(index);
        }
        let group = self.create_setting_group(setting);
        S::store_mut(self).kinds[setting.index()]
            .keys
            .insert(hash, group.index);
        group
    }

    /// Reserve room for `n` more groups of `setting` created by key.
    fn reserve_setting_groups<S: SettingKind>(&mut self, setting: S, n: usize) {
        let store = S::store_mut(self);
        store.kinds[setting.index()].keys.reserve(n);
        store.groups.reserve(n);
    }

    /// The group of `setting` created for `key`, if any.
    fn setting_group_by_key<S: SettingKind>(
        &self,
        setting: S,
        key: impl Hash,
    ) -> Option<SettingGroupId<S>> {
        S::store(self).kinds[setting.index()]
            .keys
            .get(&one_shot_128(&key))
            .copied()
            .map(SettingGroupId::new)
    }

    /// Make `setting` a single group that every person is in, now and later,
    /// without storing a member list; contacts are sampled from the whole
    /// population. Anyone added to another group of this kind leaves it.
    fn create_population_setting_group<S: SettingKind>(&mut self, setting: S) -> SettingGroupId<S> {
        let group = self.create_setting_group(setting);
        S::store_mut(self).kinds[setting.index()].population_group = Some(group.index);
        group
    }

    /// Add `person` to `group`, leaving their previous group of that kind.
    /// Adding them to a group they are in is a no-op.
    fn add_setting_member<S: SettingKind>(&mut self, group: SettingGroupId<S>, person: PersonId) {
        let store = S::store_mut(self);
        let setting = S::ALL[store.groups[group.index as usize].kind];
        let m = store.member(person, setting);
        if m.group == group.index && m.position != NOT_ACTIVE {
            return;
        }
        store.remove(person, setting);
        if store.kinds[setting.index()].population_group == Some(group.index) {
            return;
        }
        let target = &mut store.groups[group.index as usize];
        let position = u32::try_from(target.members.len()).expect("setting group too large");
        target
            .members
            .push(u32::try_from(person.0).expect("too many people"));
        target.resized();
        *store.member_mut(person, setting) = Member {
            group: group.index,
            position,
        };
    }

    /// Take `person` out of their group of `setting`, e.g. for an
    /// intervention. The group remembers its largest size, so the bound holds
    /// when [`restore_setting_member`](Self::restore_setting_member) puts
    /// them back.
    ///
    /// # Panics
    ///
    /// If that is a population group, which nobody can leave.
    fn remove_setting_member<S: SettingKind>(&mut self, person: PersonId, setting: S) {
        let store = S::store_mut(self);
        assert!(
            store.member(person, setting).group != NO_GROUP
                || store.kinds[setting.index()].population_group.is_none(),
            "cannot remove {person:?} from the population group of setting {}",
            setting.name()
        );
        store.remove(person, setting);
    }

    /// Put `person` back in the group of `setting` they were removed from.
    /// No-op if they are in it or never had one.
    fn restore_setting_member<S: SettingKind>(&mut self, person: PersonId, setting: S) {
        let member = S::store(self).member(person, setting);
        if member.group != NO_GROUP && member.position == NOT_ACTIVE {
            self.add_setting_member(SettingGroupId::<S>::new(member.group), person);
        }
    }

    /// `person`'s group of `setting`, if they are in one now.
    fn setting_group<S: SettingKind>(
        &self,
        person: PersonId,
        setting: S,
    ) -> Option<SettingGroupId<S>> {
        S::store(self)
            .group_of(person, setting)
            .map(SettingGroupId::new)
    }

    /// Number of people in `person`'s group of `setting` now; 0 without one.
    fn setting_group_size<S: SettingKind>(&self, person: PersonId, setting: S) -> usize {
        let store = S::store(self);
        let Some(group) = store.group_of(person, setting) else {
            return 0;
        };
        if store.kinds[setting.index()].population_group == Some(group) {
            self.get_entity_count::<Person>()
        } else {
            store.groups[group as usize].members.len()
        }
    }

    /// Where itineraries come from for everyone without one of their own: an
    /// [`Itinerary`] for all, or an [`ItineraryAssignment`] by property value
    /// or per person.
    fn set_itinerary_assignment<S: SettingKind>(
        &mut self,
        assignment: impl IntoItineraryAssignment<S>,
    ) {
        S::store_mut(self).assignment = Some(assignment.into_itinerary_assignment());
    }

    /// `person`'s own itinerary, overriding the assignment.
    fn set_itinerary<S: SettingKind>(&mut self, person: PersonId, itinerary: Itinerary<S>) {
        S::store_mut(self).itineraries.insert(person, itinerary);
    }

    /// `person`'s itinerary: their own, else the assignment's, else `None`.
    fn itinerary<S: SettingKind>(&self, person: PersonId) -> Option<Itinerary<S>>;

    /// Factor on `person`'s intrinsic infectiousness for an attempt in
    /// `setting`: their group's current multiplier, 0 if they have no group of
    /// it.
    fn setting_multiplier<S: SettingKind>(&self, person: PersonId, setting: S) -> f64 {
        let population = self.get_entity_count::<Person>();
        S::store(self)
            .multipliers(person, setting, population)
            .map_or(0.0, |(current, _)| current)
    }

    /// Expected factor on `person`'s intrinsic infectiousness now: the
    /// itinerary weighted average of their groups' multipliers, over the kinds
    /// they are in a group of; 0 without an itinerary or any group. The loop
    /// uses [`setting_multiplier`](Self::setting_multiplier) of the drawn
    /// setting instead, which has this mean.
    fn current_setting_multiplier<S: SettingKind>(&self, person: PersonId) -> f64 {
        let Some(itinerary) = self.itinerary::<S>(person) else {
            return 0.0;
        };
        let store = S::store(self);
        let population = self.get_entity_count::<Person>();
        let (mut weighted, mut ratio_sum) = (0.0, 0.0);
        for (setting, ratio) in itinerary.entries() {
            if ratio > 0.0 {
                if let Some((current, _)) = store.multipliers(person, setting, population) {
                    weighted += ratio * current;
                    ratio_sum += ratio;
                }
            }
        }
        if ratio_sum > 0.0 {
            weighted / ratio_sum
        } else {
            0.0
        }
    }

    /// Upper bound on [`setting_multiplier`](Self::setting_multiplier) for any
    /// setting [`sample_setting`](Self::sample_setting) can draw this run: the
    /// largest multiplier any group in `person`'s itinerary has ever had,
    /// including groups they were removed from. Valid as long as their
    /// itinerary does not move time into a kind it gave no time to when this
    /// was called.
    fn max_setting_multiplier<S: SettingKind>(&self, person: PersonId) -> f64 {
        let Some(itinerary) = self.itinerary::<S>(person) else {
            return 0.0;
        };
        let store = S::store(self);
        let population = self.get_entity_count::<Person>();
        itinerary
            .entries()
            .filter(|&(_, ratio)| ratio > 0.0)
            .filter_map(|(setting, _)| {
                let group = store.remembered_group_of(person, setting)?;
                Some(store.group_multipliers(group, setting, population).1)
            })
            .fold(0.0, f64::max)
    }

    /// Draw where an attempt by `person` happens: by itinerary ratio over the
    /// kinds they are in a group of. `None` without an itinerary or any group.
    fn sample_setting<S: SettingKind>(&self, person: PersonId) -> Option<S> {
        let itinerary = self.itinerary::<S>(person)?;
        let store = S::store(self);
        let mut ratios = [0.0; MAX_SETTINGS];
        for (setting, ratio) in itinerary.entries() {
            if ratio > 0.0 && store.group_of(person, setting).is_some() {
                ratios[setting.index()] = ratio;
            }
        }
        self.sample(SettingsRng, |rng| draw_setting(&ratios, rng.random()))
    }

    /// A random member of `person`'s group of `setting` other than `person`,
    /// or `None` if there is nobody.
    fn sample_setting_contact<S: SettingKind>(
        &self,
        person: PersonId,
        setting: S,
    ) -> Option<PersonId>;
}

impl SettingsExt for Context {
    fn itinerary<S: SettingKind>(&self, person: PersonId) -> Option<Itinerary<S>> {
        S::store(self).itinerary(self, person)
    }

    fn sample_setting_contact<S: SettingKind>(
        &self,
        person: PersonId,
        setting: S,
    ) -> Option<PersonId> {
        let store = S::store(self);
        self.sample(SettingsRng, |rng| {
            store.sample_contact(self, rng, person, setting)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        draw_setting, Itinerary, ItineraryAssignment, SettingGroupId, SettingKind, SettingsExt,
        MAX_SETTINGS,
    };
    use crate::core_epi::person::{Person, PersonId};
    use crate::prelude::*;

    define_property!(
        enum AgeGroup {
            Child,
            Adult,
        },
        Person,
        default_const = AgeGroup::Adult
    );

    define_settings!(Setting {
        Home,
        Work,
        Community
    });

    fn context_with(n: usize) -> (Context, Vec<PersonId>) {
        let mut context = Context::new();
        context.init_random(7);
        context.register_setting(Setting::Home, 1.0);
        context.register_setting(Setting::Work, 0.5);
        context.register_setting(Setting::Community, 0.0);
        let people = (0..n)
            .map(|_| context.add_entity(Person).unwrap())
            .collect();
        (context, people)
    }

    fn home_for_all(context: &mut Context, people: &[PersonId]) -> SettingGroupId<Setting> {
        let home = context.create_setting_group(Setting::Home);
        for &p in people {
            context.add_setting_member(home, p);
        }
        home
    }

    #[test]
    fn kind_index_and_name() {
        assert_eq!(
            Setting::ALL,
            &[Setting::Home, Setting::Work, Setting::Community]
        );
        assert_eq!(Setting::Work.index(), 1);
        assert_eq!(Setting::Community.name(), "Community");
    }

    #[test]
    fn multiplier_follows_alpha() {
        let (mut context, people) = context_with(4);
        home_for_all(&mut context, &people);
        let work = context.create_setting_group(Setting::Work);
        for &p in &people {
            context.add_setting_member(work, p);
        }
        let community = context.create_population_setting_group(Setting::Community);
        assert_eq!(
            context.setting_group(people[0], Setting::Community),
            Some(community)
        );
        assert_eq!(context.setting_group_size(people[0], Setting::Community), 4);
        for (setting, expected) in [
            (Setting::Home, 3.0),
            (Setting::Work, 3f64.sqrt()),
            (Setting::Community, 1.0),
        ] {
            context.set_itinerary(people[0], Itinerary::new([(setting, 1.0)]));
            assert_eq!(
                context.current_setting_multiplier::<Setting>(people[0]),
                expected
            );
            assert_eq!(
                context.max_setting_multiplier::<Setting>(people[0]),
                expected
            );
        }
    }

    #[test]
    fn current_multiplier_is_the_itinerary_weighted_average() {
        let (mut context, people) = context_with(3);
        home_for_all(&mut context, &people);
        context.create_population_setting_group(Setting::Community);
        context.set_itinerary_assignment(Itinerary::new([
            (Setting::Home, 0.25),
            (Setting::Community, 0.75),
        ]));
        // 0.25 * 2 + 0.75 * 1
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[0]),
            1.25
        );
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 2.0);
    }

    #[test]
    fn no_group_means_no_contacts() {
        let (mut context, people) = context_with(2);
        let home = context.create_setting_group(Setting::Home);
        context.add_setting_member(home, people[0]);
        context.set_itinerary_assignment(Itinerary::new([(Setting::Home, 1.0)]));
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[1]),
            0.0
        );
        assert_eq!(context.sample_setting::<Setting>(people[1]), None);
        assert_eq!(
            context.sample_setting_contact(people[1], Setting::Home),
            None
        );
        // Alone in the group.
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[0]),
            0.0
        );
        assert_eq!(
            context.sample_setting_contact(people[0], Setting::Home),
            None
        );
    }

    #[test]
    fn moving_groups_updates_both_member_lists() {
        let (mut context, people) = context_with(3);
        let a = home_for_all(&mut context, &people);
        let b = context.create_setting_group(Setting::Home);
        context.add_setting_member(b, people[2]);
        context.add_setting_member(b, people[2]);
        assert_eq!(context.setting_group(people[2], Setting::Home), Some(b));
        assert_eq!(context.setting_group(people[0], Setting::Home), Some(a));
        assert_eq!(context.setting_group_size(people[0], Setting::Home), 2);
        assert_eq!(context.setting_group_size(people[2], Setting::Home), 1);
        for _ in 0..20 {
            let contact = context
                .sample_setting_contact(people[0], Setting::Home)
                .unwrap();
            assert_eq!(contact, people[1]);
        }
    }

    #[test]
    fn removing_members_lowers_the_current_but_not_the_bound() {
        let (mut context, people) = context_with(3);
        let home = home_for_all(&mut context, &people);
        context.set_itinerary_assignment(Itinerary::new([(Setting::Home, 1.0)]));
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 2.0);

        context.remove_setting_member(people[1], Setting::Home);
        assert_eq!(context.setting_group(people[1], Setting::Home), None);
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[0]),
            1.0
        );
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 2.0);
        assert_eq!(
            context.sample_setting_contact(people[0], Setting::Home),
            Some(people[2])
        );

        context.add_setting_member(home, people[1]);
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[0]),
            2.0
        );
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 2.0);
    }

    #[test]
    fn restore_puts_a_member_back_in_the_remembered_group() {
        let (mut context, people) = context_with(3);
        let home = home_for_all(&mut context, &people);
        context.remove_setting_member(people[1], Setting::Home);
        context.remove_setting_member(people[1], Setting::Home);
        assert_eq!(context.setting_group(people[1], Setting::Home), None);
        assert_eq!(context.setting_group_size(people[0], Setting::Home), 2);
        context.restore_setting_member(people[1], Setting::Home);
        assert_eq!(context.setting_group(people[1], Setting::Home), Some(home));
        assert_eq!(context.setting_group_size(people[0], Setting::Home), 3);
        // Never a member: nothing to restore.
        context.register_setting(Setting::Work, 1.0);
        context.restore_setting_member(people[1], Setting::Work);
        assert_eq!(context.setting_group(people[1], Setting::Work), None);
    }

    #[test]
    fn itinerary_renormalizes_over_the_kinds_a_person_has() {
        let (mut context, people) = context_with(3);
        home_for_all(&mut context, &people[..2]);
        context.register_setting(Setting::Work, 1.0);
        let work = context.create_setting_group(Setting::Work);
        for &p in &people {
            context.add_setting_member(work, p);
        }
        context
            .set_itinerary_assignment(Itinerary::new([(Setting::Home, 0.5), (Setting::Work, 0.5)]));
        // Home has 1 other, work 2: average 1.5.
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[0]),
            1.5
        );
        // No home: all of the time is at work.
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[2]),
            2.0
        );
        // Removed from work: all at home, while the bound keeps the workplace.
        context.remove_setting_member(people[0], Setting::Work);
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[0]),
            1.0
        );
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 2.0);
        for _ in 0..20 {
            assert_eq!(
                context.sample_setting::<Setting>(people[0]),
                Some(Setting::Home)
            );
        }
    }

    #[test]
    fn population_group_samples_anyone_else() {
        let (mut context, people) = context_with(3);
        context.create_population_setting_group(Setting::Community);
        for _ in 0..20 {
            let contact = context
                .sample_setting_contact(people[0], Setting::Community)
                .unwrap();
            assert_ne!(contact, people[0]);
        }
    }

    #[test]
    #[should_panic(expected = "population group")]
    fn cannot_leave_a_population_group() {
        let (mut context, people) = context_with(1);
        context.create_population_setting_group(Setting::Community);
        context.remove_setting_member(people[0], Setting::Community);
    }

    #[test]
    fn itinerary_normalizes() {
        let itinerary = Itinerary::new([(Setting::Home, 2.0), (Setting::Community, 6.0)]);
        assert_eq!(itinerary.ratio(Setting::Home), 0.25);
        assert_eq!(itinerary.ratio(Setting::Work), 0.0);
        assert_eq!(itinerary.ratio(Setting::Community), 0.75);
    }

    #[test]
    fn draw_setting_by_weight() {
        let mut weights = [0.0; MAX_SETTINGS];
        assert_eq!(draw_setting::<Setting>(&weights, 0.5), None);
        weights[Setting::Home.index()] = 1.0;
        weights[Setting::Community.index()] = 3.0;
        assert_eq!(draw_setting::<Setting>(&weights, 0.0), Some(Setting::Home));
        assert_eq!(
            draw_setting::<Setting>(&weights, 0.2499),
            Some(Setting::Home)
        );
        assert_eq!(
            draw_setting::<Setting>(&weights, 0.25),
            Some(Setting::Community)
        );
        assert_eq!(
            draw_setting::<Setting>(&weights, 0.9999),
            Some(Setting::Community)
        );
    }

    #[test]
    fn sample_setting_draws_by_ratio_over_kinds_with_a_group() {
        let (mut context, people) = context_with(1);
        home_for_all(&mut context, &people);
        context.register_setting(Setting::Work, 1.0);
        context
            .set_itinerary_assignment(Itinerary::new([(Setting::Home, 0.1), (Setting::Work, 0.9)]));
        // No workplace, so every attempt is at home, even alone there: the
        // multiplier, not the draw, thins it out.
        for _ in 0..20 {
            assert_eq!(context.sample_setting(people[0]), Some(Setting::Home));
        }
        assert_eq!(context.setting_multiplier(people[0], Setting::Home), 0.0);
        assert_eq!(context.setting_multiplier(people[0], Setting::Work), 0.0);
    }

    #[test]
    fn personal_itinerary_overrides_the_assignment() {
        let (mut context, people) = context_with(3);
        home_for_all(&mut context, &people);
        context.create_population_setting_group(Setting::Community);
        assert_eq!(context.sample_setting::<Setting>(people[0]), None);
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 0.0);

        context.set_itinerary_assignment(Itinerary::new([(Setting::Community, 1.0)]));
        context.set_itinerary(people[1], Itinerary::new([(Setting::Home, 1.0)]));
        assert_eq!(context.sample_setting(people[0]), Some(Setting::Community));
        assert_eq!(context.sample_setting(people[1]), Some(Setting::Home));
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 1.0);
        assert_eq!(context.max_setting_multiplier::<Setting>(people[1]), 2.0);
    }

    #[test]
    fn itinerary_assigned_by_property_follows_the_current_value() {
        let (mut context, people) = context_with(2);
        home_for_all(&mut context, &people);
        context.create_population_setting_group(Setting::Community);
        let child = Itinerary::new([(Setting::Home, 1.0)]);
        let adult = Itinerary::new([(Setting::Community, 1.0)]);
        context.set_itinerary_assignment(ItineraryAssignment::by_property(move |age: AgeGroup| {
            match age {
                AgeGroup::Child => child,
                AgeGroup::Adult => adult,
            }
        }));
        context.set_property(people[0], AgeGroup::Child);
        assert_eq!(context.sample_setting(people[0]), Some(Setting::Home));
        assert_eq!(context.sample_setting(people[1]), Some(Setting::Community));
        context.set_property(people[0], AgeGroup::Adult);
        assert_eq!(context.sample_setting(people[0]), Some(Setting::Community));
    }

    #[test]
    fn itinerary_assigned_individually_by_closure() {
        let (mut context, people) = context_with(2);
        context.create_population_setting_group(Setting::Community);
        context
            .set_itinerary_assignment(|_: &Context, _| Itinerary::new([(Setting::Community, 1.0)]));
        assert_eq!(context.sample_setting(people[1]), Some(Setting::Community));
    }

    #[test]
    fn contact_needs_someone_else_in_the_setting() {
        let (mut context, people) = context_with(3);
        home_for_all(&mut context, &people);
        context.register_setting(Setting::Work, 1.0);
        let work = context.create_setting_group(Setting::Work);
        context.add_setting_member(work, people[0]);
        context
            .set_itinerary_assignment(Itinerary::new([(Setting::Home, 0.5), (Setting::Work, 0.5)]));
        // Alone at work: no contact there, and the multiplier is zero.
        assert_eq!(context.setting_multiplier(people[0], Setting::Work), 0.0);
        assert_eq!(
            context.sample_setting_contact(people[0], Setting::Work),
            None
        );
        assert_eq!(context.setting_multiplier(people[0], Setting::Home), 2.0);
        for _ in 0..20 {
            let contact = context
                .sample_setting_contact(people[0], Setting::Home)
                .unwrap();
            assert_ne!(contact, people[0]);
        }
        context.remove_setting_member(people[0], Setting::Home);
        assert_eq!(context.sample_setting(people[0]), Some(Setting::Work));
        assert_eq!(context.setting_multiplier(people[0], Setting::Home), 0.0);
    }

    #[test]
    fn groups_by_key_are_created_once_per_kind() {
        let mut context = Context::new();
        context.register_setting(Setting::Home, 1.0);
        context.register_setting(Setting::Work, 1.0);
        let a = context.setting_group_for_key(Setting::Home, 7u64);
        let b = context.setting_group_for_key(Setting::Home, 8u64);
        assert_ne!(a, b);
        assert_eq!(context.setting_group_for_key(Setting::Home, 7u64), a);
        assert_ne!(
            context.setting_group_for_key(Setting::Work, 7u64).index,
            a.index
        );
        assert_eq!(context.setting_group_by_key(Setting::Home, 8u64), Some(b));
        assert_eq!(context.setting_group_by_key(Setting::Home, 9u64), None);
        // Owned and borrowed strings are the same key.
        let s = context.setting_group_for_key(Setting::Home, "HH1".to_string());
        assert_eq!(context.setting_group_by_key(Setting::Home, "HH1"), Some(s));
    }

    #[test]
    #[should_panic(expected = "not registered")]
    fn unregistered_setting_panics() {
        let mut context = Context::new();
        context.create_setting_group(Setting::Home);
    }
}
