//! Settings: named kinds of place (household, school, ...) whose instances
//! group people, with a density exponent per kind and an itinerary per person
//! saying how their time is split across kinds.
//!
//! Define the kind type with [`define_settings!`](crate::define_settings), then
//! at setup add each kind with its name and `alpha`
//! ([`add_setting_kind`](SettingsExt::add_setting_kind)), create groups and add members (by
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
//! any of their groups has ever had, whatever their itinerary, so members can
//! be removed and added back and itineraries overridden and cleared
//! (interventions) without re-forecasting. A group growing past its largest
//! size (for a population group with `alpha > 0`, any new person), or a
//! person joining a new group, can exceed it: then call
//! [`reforecast_transmission`](super::TransmissionManager::reforecast_transmission)
//! for the infectious people affected.
//!
//! A model's [`TransmissionManager`](super::TransmissionManager) can use
//! them in two ways, which give attempts in each setting at the same rate,
//! `intrinsic * ratio * multiplier`:
//!
//! - Exact, when nothing changes while a person is infectious (or every
//!   change is followed by a reforecast): `contact_multiplier` and
//!   `max_contact_multiplier` are both
//!   [`current_setting_multiplier`](SettingsExt::current_setting_multiplier),
//!   so every proposal is an attempt, and `sample_contact` draws the setting
//!   with [`sample_setting_by_rate`](SettingsExt::sample_setting_by_rate) (by
//!   ratio times multiplier) and a contact there with
//!   [`sample_setting_contact`](SettingsExt::sample_setting_contact).
//! - Setting first, for populations whose memberships or itineraries change:
//!   `max_contact_multiplier` is
//!   [`max_setting_multiplier`](SettingsExt::max_setting_multiplier), and
//!   `evaluate_forecast` draws the setting with
//!   [`sample_setting`](SettingsExt::sample_setting) (by itinerary ratio),
//!   thins by that setting's
//!   [`setting_multiplier`](SettingsExt::setting_multiplier), and samples a
//!   contact there. Proposals are rejected wherever a setting's multiplier is
//!   below the person's largest over every kind, including kinds their
//!   itinerary gives no time to.
//!
//! Either path can return the setting as the exposure, so that `on_infected`
//! knows where a transmission happened.
//!
//! Either way a proposal reads the person's membership records (one per
//! kind, person-major so they share a cache line) and a group record per
//! kind for the multipliers, which are cached per group. Nothing is hashed
//! or allocated; itineraries are small `Copy` values.

use std::cell::Cell;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

use smallvec::SmallVec;

use crate::data_structures::entity_map::EntityMap;
use crate::entity::property::Property;
use crate::entity::{Entity, EntityId};
use crate::hashing::one_shot_128;
use crate::rand::{Rng, RngExt};
use crate::random::sample_single_excluding;
use crate::{define_rng, Context, ContextBase, ContextEntitiesExt, ContextRandomExt, HashMap};

define_rng!(CoreEpiSettingsRng);

/// The most kinds one setting type may have.
pub const MAX_SETTINGS: usize = 8;

/// A kind of setting (household, school, ...): a value of the type made by
/// [`define_settings!`](crate::define_settings), added at run time with
/// [`add_setting_kind`](SettingsExt::add_setting_kind).
pub trait SettingKind: Copy + Eq + Debug + 'static {
    /// The entity whose members the groups hold, e.g. the model's `Person`.
    type Entity: Entity;

    /// Position among the kinds, in the order they were added.
    fn index(self) -> usize;

    #[doc(hidden)]
    fn from_index(index: usize) -> Self;

    #[doc(hidden)]
    fn store<C: ContextBase>(context: &C) -> &SettingsStore<Self>;

    #[doc(hidden)]
    fn store_mut<C: ContextBase>(context: &mut C) -> &mut SettingsStore<Self>;
}

/// Defines the type of an entity's setting kinds and the storage behind it.
/// The kinds themselves are added at run time with
/// [`add_setting_kind`](SettingsExt::add_setting_kind), so they can come from
/// a parameters file.
///
/// ```ignore
/// define_settings!(pub Setting for Person);
/// ```
#[macro_export]
macro_rules! define_settings {
    ($vis:vis $name:ident for $entity:ty) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        $vis struct $name(u8);

        $crate::paste::paste! {
            $crate::define_data_plugin!(
                [<$name SettingsPlugin>],
                $crate::core_epi::settings::SettingsStore<$name>,
                $crate::core_epi::settings::SettingsStore::new()
            );

            impl $crate::core_epi::settings::SettingKind for $name {
                type Entity = $entity;

                fn index(self) -> usize {
                    self.0 as usize
                }

                fn from_index(index: usize) -> Self {
                    Self(u8::try_from(index).expect("too many setting kinds"))
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

/// How a person's time is split across setting kinds: shares of where their
/// attempts happen, not a scale on how many. Proportions are normalized, so
/// they need not sum to one; a model whose proportions also carry a scale
/// multiplies their sum into its total and its forecast. A small `Copy`
/// value.
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
                "proportion for setting {setting:?} must be finite and non-negative, got {proportion}"
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

    /// The kinds with a positive proportion, each with its normalized
    /// proportion.
    pub fn entries(&self) -> impl Iterator<Item = (S, f64)> + '_ {
        self.ratios
            .iter()
            .enumerate()
            .filter(|&(_, &ratio)| ratio > 0.0)
            .map(|(index, &ratio)| (S::from_index(index), ratio))
    }
}

type IndividualFn<S> = Box<dyn Fn(&Context, EntityId<<S as SettingKind>::Entity>) -> Itinerary<S>>;

/// Where people's itineraries come from: one for everyone, one per value of a
/// property, or one computed per person. Resolved on every lookup.
pub struct ItineraryAssignment<S: SettingKind>(Assign<S>);

enum Assign<S: SettingKind> {
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
        P: Property<S::Entity>,
        F: Fn(P) -> Itinerary<S> + 'static,
    {
        Self::individual(move |context, person| f(context.get_property(person)))
    }

    /// An itinerary per person.
    pub fn individual<F>(f: F) -> Self
    where
        F: Fn(&Context, EntityId<S::Entity>) -> Itinerary<S> + 'static,
    {
        Self(Assign::Individual(Box::new(f)))
    }

    fn resolve(&self, context: &Context, person: EntityId<S::Entity>) -> Itinerary<S> {
        match &self.0 {
            Assign::Global(itinerary) => *itinerary,
            Assign::Individual(f) => f(context, person),
        }
    }
}

/// Anything [`set_itinerary_assignment`](SettingsExt::set_itinerary_assignment)
/// accepts: an [`ItineraryAssignment`], a bare [`Itinerary`] (global), or a
/// closure `Fn(&Context, EntityId<S::Entity>) -> Itinerary<S>` (individual).
pub trait IntoItineraryAssignment<S: SettingKind> {
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
    F: Fn(&Context, EntityId<S::Entity>) -> Itinerary<S> + 'static,
{
    fn into_itinerary_assignment(self) -> ItineraryAssignment<S> {
        ItineraryAssignment::individual(self)
    }
}

/// `(n - 1)^alpha` for a group of `n`; 0 for an empty group or a person
/// alone, whatever `alpha`. The common exponents skip the `powf`.
fn density_multiplier(alpha: f64, n: usize) -> f64 {
    if n <= 1 {
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

/// A group's `(n - 1)^alpha` at its current and at its largest ever size.
#[derive(Clone, Copy)]
struct Multipliers {
    current: f64,
    largest: f64,
}

struct Kind {
    name: String,
    alpha: f64,
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
    /// Computed on the first read after a membership change, so that bulk
    /// loading pays no `powf` per member.
    cached_multipliers: Cell<Option<Multipliers>>,
}

impl Group {
    fn resized(&mut self) {
        self.max_size = self.max_size.max(self.members.len());
        self.cached_multipliers.set(None);
    }

    fn multipliers(&self, alpha: f64) -> Multipliers {
        if let Some(cached) = self.cached_multipliers.get() {
            return cached;
        }
        let multipliers = Multipliers {
            current: density_multiplier(alpha, self.members.len()),
            largest: density_multiplier(alpha, self.max_size),
        };
        self.cached_multipliers.set(Some(multipliers));
        multipliers
    }
}

/// Storage for one setting type; defined per type by
/// [`define_settings!`](crate::define_settings).
pub struct SettingsStore<S: SettingKind> {
    kinds: Vec<Kind>,
    /// One `Member` per person per kind, person-major with stride
    /// `kinds.len()`, so an attempt's lookups across kinds share a cache line.
    memberships: Vec<Member>,
    groups: Vec<Group>,
    assignment: Option<ItineraryAssignment<S>>,
    /// Per-person overrides of the assignment.
    itineraries: EntityMap<S::Entity, Itinerary<S>>,
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
        Self {
            kinds: Vec::new(),
            memberships: Vec::new(),
            groups: Vec::new(),
            assignment: None,
            itineraries: EntityMap::new(),
        }
    }

    /// Add a kind, widening every person's membership records by one.
    fn add_kind(&mut self, name: String, alpha: f64) -> S {
        assert!(
            self.kinds.len() < MAX_SETTINGS,
            "at most {MAX_SETTINGS} setting kinds are supported"
        );
        assert!(
            self.kinds.iter().all(|kind| kind.name != name),
            "setting kind {name} already exists"
        );
        let old_stride = self.kinds.len();
        if !self.memberships.is_empty() {
            let mut widened =
                Vec::with_capacity(self.memberships.len() / old_stride * (old_stride + 1));
            for person in self.memberships.chunks(old_stride) {
                widened.extend_from_slice(person);
                widened.push(Member::NONE);
            }
            self.memberships = widened;
        }
        self.kinds.push(Kind {
            name,
            alpha,
            population_group: None,
            keys: HashMap::default(),
        });
        S::from_index(old_stride)
    }

    fn alpha(&self, setting: S) -> f64 {
        self.kinds[setting.index()].alpha
    }

    /// `person`'s membership record of `setting`; `Member::NONE` if they never
    /// had a group of it.
    fn member(&self, person: EntityId<S::Entity>, setting: S) -> Member {
        debug_assert!(setting.index() < self.kinds.len(), "unknown setting kind");
        self.memberships
            .get(person.0 * self.kinds.len() + setting.index())
            .copied()
            .unwrap_or(Member::NONE)
    }

    fn member_mut(&mut self, person: EntityId<S::Entity>, setting: S) -> &mut Member {
        let stride = self.kinds.len();
        debug_assert!(setting.index() < stride, "unknown setting kind");
        let index = person.0 * stride + setting.index();
        if index >= self.memberships.len() {
            self.memberships
                .resize((person.0 + 1) * stride, Member::NONE);
        }
        &mut self.memberships[index]
    }

    /// The group `person` is in now, if any.
    fn group_of(&self, person: EntityId<S::Entity>, setting: S) -> Option<u32> {
        let member = self.member(person, setting);
        if member.group == NO_GROUP {
            self.kinds[setting.index()].population_group
        } else {
            (member.position != NOT_ACTIVE).then_some(member.group)
        }
    }

    /// The group `person` is in or was removed from, if any.
    fn remembered_group_of(&self, person: EntityId<S::Entity>, setting: S) -> Option<u32> {
        let member = self.member(person, setting);
        if member.group == NO_GROUP {
            self.kinds[setting.index()].population_group
        } else {
            Some(member.group)
        }
    }

    /// The current multiplier of the group `person` is in now, given the
    /// population size for a population group; `None` without a group.
    fn current_multiplier(
        &self,
        person: EntityId<S::Entity>,
        setting: S,
        population: usize,
    ) -> Option<f64> {
        let group = self.group_of(person, setting)?;
        Some(self.group_multipliers(group, setting, population).current)
    }

    fn group_multipliers(&self, group: u32, setting: S, population: usize) -> Multipliers {
        if self.kinds[setting.index()].population_group == Some(group) {
            // A population group has no largest-ever size: see
            // `max_setting_multiplier`.
            let multiplier = density_multiplier(self.alpha(setting), population);
            Multipliers {
                current: multiplier,
                largest: multiplier,
            }
        } else {
            self.groups[group as usize].multipliers(self.alpha(setting))
        }
    }

    fn itinerary(&self, context: &Context, person: EntityId<S::Entity>) -> Option<Itinerary<S>> {
        self.itineraries
            .get(person)
            .copied()
            .or_else(|| Some(self.assignment.as_ref()?.resolve(context, person)))
    }

    /// Take `person` out of their current list group of `setting`, if any,
    /// remembering the group.
    fn remove(&mut self, person: EntityId<S::Entity>, setting: S) {
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
            self.member_mut(EntityId::new(moved as usize), setting)
                .position = member.position;
        }
    }

    fn sample_contact<R: Rng>(
        &self,
        context: &Context,
        rng: &mut R,
        person: EntityId<S::Entity>,
        setting: S,
    ) -> Option<EntityId<S::Entity>> {
        let group = self.group_of(person, setting)?;
        if self.kinds[setting.index()].population_group == Some(group) {
            context
                .query(S::Entity::default())
                .sample_entity_excluding(rng, person)
        } else {
            sample_single_excluding(rng, &self.groups[group as usize].members, person.0 as u32)
                .map(|&i| EntityId::new(i as usize))
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
    for (index, &w) in weights.iter().enumerate() {
        if w > 0.0 {
            acc += w;
            last = Some(S::from_index(index));
            if u * total < acc {
                return last;
            }
        }
    }
    last
}

/// Setting membership, itineraries and the quantities the transmission loop
/// needs from them. `S` is inferred from a setting or group argument;
/// methods without one take it as a type parameter
/// (`context.sample_setting::<Setting>(person)`).
pub trait SettingsExt: ContextBase + ContextRandomExt + ContextEntitiesExt {
    /// Add a kind of setting called `name` with density exponent `alpha`.
    /// Kinds can be added at any time, up to [`MAX_SETTINGS`].
    ///
    /// ```
    /// # use ixa::core_epi::SettingsExt;
    /// # use ixa::prelude::*;
    /// # define_entity!(Person);
    /// ixa::define_settings!(Place for Person);
    /// let mut context = Context::new();
    /// let household = context.add_setting_kind::<Place>("household", 1.0);
    /// let person = context.add_entity(Person).unwrap();
    /// let home = context.create_setting_group(household);
    /// context.add_setting_member(home, person);
    /// assert_eq!(context.setting_kind_by_name::<Place>("household"), Some(household));
    /// assert_eq!(context.setting_name(household), "household");
    /// ```
    ///
    /// # Panics
    ///
    /// If `alpha` is negative or not finite, the name is taken, or there are
    /// already [`MAX_SETTINGS`] kinds.
    fn add_setting_kind<S: SettingKind>(&mut self, name: &str, alpha: f64) -> S {
        assert!(
            alpha.is_finite() && alpha >= 0.0,
            "alpha for setting {name} must be finite and non-negative, got {alpha}"
        );
        S::store_mut(self).add_kind(name.to_string(), alpha)
    }

    /// The kinds of `S`, in the order they were added.
    fn setting_kinds<S: SettingKind>(&self) -> Vec<S> {
        (0..S::store(self).kinds.len()).map(S::from_index).collect()
    }

    /// The kind called `name`, if one was added.
    fn setting_kind_by_name<S: SettingKind>(&self, name: &str) -> Option<S> {
        let index = S::store(self)
            .kinds
            .iter()
            .position(|kind| kind.name == name)?;
        Some(S::from_index(index))
    }

    /// The name of `setting`.
    fn setting_name<S: SettingKind>(&self, setting: S) -> &str {
        &S::store(self).kinds[setting.index()].name
    }

    /// Create an empty group of `setting`.
    fn create_setting_group<S: SettingKind>(&mut self, setting: S) -> SettingGroupId<S> {
        let store = S::store_mut(self);
        store.groups.push(Group {
            kind: setting.index(),
            members: SmallVec::new(),
            max_size: 0,
            cached_multipliers: Cell::new(None),
        });
        SettingGroupId::new(u32::try_from(store.groups.len() - 1).expect("too many setting groups"))
    }

    /// The group of `setting` with external `key`, created on first use: for
    /// loading a line list whose rows carry setting ids. Use one key type per
    /// kind; keys are compared by hash.
    ///
    /// ```
    /// # use ixa::core_epi::SettingsExt;
    /// # use ixa::prelude::*;
    /// # define_entity!(Person);
    /// # ixa::define_settings!(Setting for Person);
    /// # let mut context = Context::new();
    /// # let household_kind = context.add_setting_kind::<Setting>("household", 1.0);
    /// let line_list = "person_id,household_id\n1,HH001\n2,HH001\n3,HH002\n";
    /// let mut people = Vec::new();
    /// for row in csv::Reader::from_reader(line_list.as_bytes()).records() {
    ///     let row = row.unwrap();
    ///     let person = context.add_entity(Person).unwrap();
    ///     let household = context.setting_group_for_key(household_kind, &row[1]);
    ///     context.add_setting_member(household, person);
    ///     people.push(person);
    /// }
    /// assert_eq!(context.setting_group_size(people[0], household_kind), 2);
    /// assert_eq!(context.setting_group_size(people[2], household_kind), 1);
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
    fn add_setting_member<S: SettingKind>(
        &mut self,
        group: SettingGroupId<S>,
        person: EntityId<S::Entity>,
    ) {
        let store = S::store_mut(self);
        let setting = S::from_index(store.groups[group.index as usize].kind);
        let member = store.member(person, setting);
        if member.group == group.index && member.position != NOT_ACTIVE {
            return;
        }
        store.remove(person, setting);
        if store.kinds[setting.index()].population_group == Some(group.index) {
            // Forget the list group they left, or it would shadow this one.
            if member.group != NO_GROUP {
                *store.member_mut(person, setting) = Member::NONE;
            }
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
    fn remove_setting_member<S: SettingKind>(&mut self, person: EntityId<S::Entity>, setting: S) {
        let store = S::store_mut(self);
        assert!(
            store.member(person, setting).group != NO_GROUP
                || store.kinds[setting.index()].population_group.is_none(),
            "cannot remove {person:?} from the population group of setting {setting:?}"
        );
        store.remove(person, setting);
    }

    /// Put `person` back in the group of `setting` they were removed from.
    /// No-op if they are in it or never had one.
    fn restore_setting_member<S: SettingKind>(&mut self, person: EntityId<S::Entity>, setting: S) {
        let member = S::store(self).member(person, setting);
        if member.group != NO_GROUP && member.position == NOT_ACTIVE {
            self.add_setting_member(SettingGroupId::<S>::new(member.group), person);
        }
    }

    /// The people in `group` now; everyone, for a population group.
    fn setting_group_members<S: SettingKind>(
        &self,
        group: SettingGroupId<S>,
    ) -> Vec<EntityId<S::Entity>> {
        let store = S::store(self);
        let record = &store.groups[group.index as usize];
        if store.kinds[record.kind].population_group == Some(group.index) {
            self.get_entity_iterator::<S::Entity>().collect()
        } else {
            record
                .members
                .iter()
                .map(|&index| EntityId::new(index as usize))
                .collect()
        }
    }

    /// `person`'s group of `setting`, if they are in one now.
    fn setting_group<S: SettingKind>(
        &self,
        person: EntityId<S::Entity>,
        setting: S,
    ) -> Option<SettingGroupId<S>> {
        S::store(self)
            .group_of(person, setting)
            .map(SettingGroupId::new)
    }

    /// Number of people in `person`'s group of `setting` now; 0 without one.
    fn setting_group_size<S: SettingKind>(&self, person: EntityId<S::Entity>, setting: S) -> usize {
        let store = S::store(self);
        let Some(group) = store.group_of(person, setting) else {
            return 0;
        };
        if store.kinds[setting.index()].population_group == Some(group) {
            self.get_entity_count::<S::Entity>()
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
    fn set_itinerary<S: SettingKind>(
        &mut self,
        person: EntityId<S::Entity>,
        itinerary: Itinerary<S>,
    ) {
        S::store_mut(self).itineraries.insert(person, itinerary);
    }

    /// Remove `person`'s own itinerary, so the assignment applies again.
    fn clear_itinerary<S: SettingKind>(&mut self, person: EntityId<S::Entity>) {
        let _ = S::store_mut(self).itineraries.remove(person);
    }

    /// `person`'s itinerary: their own, else the assignment's, else `None`.
    fn itinerary<S: SettingKind>(&self, person: EntityId<S::Entity>) -> Option<Itinerary<S>>;

    /// Factor on `person`'s intrinsic infectiousness for an attempt in
    /// `setting`: their group's current multiplier, 0 if they have no group of
    /// it.
    fn setting_multiplier<S: SettingKind>(&self, person: EntityId<S::Entity>, setting: S) -> f64 {
        let population = self.get_entity_count::<S::Entity>();
        S::store(self)
            .current_multiplier(person, setting, population)
            .unwrap_or(0.0)
    }

    /// Factor on `person`'s intrinsic infectiousness now, over all their
    /// settings: the itinerary weighted average of their groups' multipliers,
    /// over the kinds they are in a group of; 0 without an itinerary or any
    /// group. This is the total for the exact path: thin by it, then draw
    /// where with [`sample_setting_by_rate`](Self::sample_setting_by_rate).
    /// It is also an exact forecast, with no proposal rejected, for a person
    /// whose memberships and itinerary do not change while they are
    /// infectious; otherwise forecast with
    /// [`max_setting_multiplier`](Self::max_setting_multiplier).
    fn current_setting_multiplier<S: SettingKind>(&self, person: EntityId<S::Entity>) -> f64 {
        let Some(itinerary) = self.itinerary::<S>(person) else {
            return 0.0;
        };
        let store = S::store(self);
        let population = self.get_entity_count::<S::Entity>();
        let (mut weighted, mut ratio_sum) = (0.0, 0.0);
        for (setting, ratio) in itinerary.entries() {
            if let Some(current) = store.current_multiplier(person, setting, population) {
                weighted += ratio * current;
                ratio_sum += ratio;
            }
        }
        if ratio_sum > 0.0 {
            weighted / ratio_sum
        } else {
            0.0
        }
    }

    /// Upper bound on `person`'s
    /// [`setting_multiplier`](Self::setting_multiplier) in any setting, and
    /// so on their
    /// [`current_setting_multiplier`](Self::current_setting_multiplier): the
    /// largest multiplier any of their groups has ever had, over every kind
    /// and including groups they were removed from. It does not depend on
    /// their itinerary, so it holds when one is set, cleared or reassigned
    /// after a forecast was made; the price is that a kind their itinerary
    /// gives no time to still counts. It does not cover a list group that
    /// later grows past its largest size, a population group with
    /// `alpha > 0` that anyone is added to, or a group joined later: after
    /// such a change, call
    /// [`reforecast_transmission`](super::TransmissionManager::reforecast_transmission)
    /// for the infectious people it affects (the newcomer, and the group's
    /// [`setting_group_members`](Self::setting_group_members)).
    fn max_setting_multiplier<S: SettingKind>(&self, person: EntityId<S::Entity>) -> f64 {
        let store = S::store(self);
        let population = self.get_entity_count::<S::Entity>();
        let mut max_multiplier = 0.0_f64;
        for index in 0..store.kinds.len() {
            let setting = S::from_index(index);
            if let Some(group) = store.remembered_group_of(person, setting) {
                let largest = store.group_multipliers(group, setting, population).largest;
                max_multiplier = max_multiplier.max(largest);
            }
        }
        max_multiplier
    }

    /// Draw where an attempt by `person` happens: by itinerary ratio over the
    /// kinds they are in a group of. `None` without an itinerary or any group.
    fn sample_setting<S: SettingKind>(&self, person: EntityId<S::Entity>) -> Option<S> {
        let itinerary = self.itinerary::<S>(person)?;
        let store = S::store(self);
        let mut ratios = [0.0; MAX_SETTINGS];
        for (setting, ratio) in itinerary.entries() {
            if store.group_of(person, setting).is_some() {
                ratios[setting.index()] = ratio;
            }
        }
        self.sample(CoreEpiSettingsRng, |rng| {
            draw_setting(&ratios, rng.random())
        })
    }

    /// Draw where an attempt by `person` happens, given that it happens: by
    /// itinerary ratio times the group's current multiplier. Use after
    /// thinning by
    /// [`current_setting_multiplier`](Self::current_setting_multiplier);
    /// [`sample_setting`](Self::sample_setting) is for thinning afterwards by
    /// the drawn setting's multiplier. `None` without an itinerary, or when
    /// every multiplier is 0.
    fn sample_setting_by_rate<S: SettingKind>(&self, person: EntityId<S::Entity>) -> Option<S> {
        let itinerary = self.itinerary::<S>(person)?;
        let store = S::store(self);
        let population = self.get_entity_count::<S::Entity>();
        let mut rates = [0.0; MAX_SETTINGS];
        for (setting, ratio) in itinerary.entries() {
            if let Some(current) = store.current_multiplier(person, setting, population) {
                rates[setting.index()] = ratio * current;
            }
        }
        self.sample(CoreEpiSettingsRng, |rng| draw_setting(&rates, rng.random()))
    }

    /// A random member of `person`'s group of `setting` other than `person`,
    /// or `None` if there is nobody.
    fn sample_setting_contact<S: SettingKind>(
        &self,
        person: EntityId<S::Entity>,
        setting: S,
    ) -> Option<EntityId<S::Entity>>;
}

impl SettingsExt for Context {
    fn itinerary<S: SettingKind>(&self, person: EntityId<S::Entity>) -> Option<Itinerary<S>> {
        S::store(self).itinerary(self, person)
    }

    fn sample_setting_contact<S: SettingKind>(
        &self,
        person: EntityId<S::Entity>,
        setting: S,
    ) -> Option<EntityId<S::Entity>> {
        let store = S::store(self);
        self.sample(CoreEpiSettingsRng, |rng| {
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
    use crate::prelude::*;

    define_entity!(Person);

    define_property!(
        enum AgeGroup {
            Child,
            Adult,
        },
        Person,
        default_const = AgeGroup::Adult
    );

    define_settings!(Setting for Person);

    // The kinds `context_with` adds, in order.
    const HOME: Setting = Setting(0);
    const WORK: Setting = Setting(1);
    const COMMUNITY: Setting = Setting(2);

    fn context_with(n: usize) -> (Context, Vec<PersonId>) {
        context_with_work_alpha(n, 0.5)
    }

    fn context_with_work_alpha(n: usize, work_alpha: f64) -> (Context, Vec<PersonId>) {
        let mut context = Context::new();
        context.init_random(7);
        assert_eq!(context.add_setting_kind::<Setting>("Home", 1.0), HOME);
        assert_eq!(
            context.add_setting_kind::<Setting>("Work", work_alpha),
            WORK
        );
        assert_eq!(
            context.add_setting_kind::<Setting>("Community", 0.0),
            COMMUNITY
        );
        let people = (0..n)
            .map(|_| context.add_entity(Person).unwrap())
            .collect();
        (context, people)
    }

    fn home_for_all(context: &mut Context, people: &[PersonId]) -> SettingGroupId<Setting> {
        let home = context.create_setting_group(HOME);
        for &p in people {
            context.add_setting_member(home, p);
        }
        home
    }

    #[test]
    fn kind_index_and_name() {
        let (context, _) = context_with(0);
        assert_eq!(
            context.setting_kinds::<Setting>(),
            vec![HOME, WORK, COMMUNITY]
        );
        assert_eq!(WORK.index(), 1);
        assert_eq!(context.setting_kind_by_name::<Setting>("Work"), Some(WORK));
        assert_eq!(context.setting_kind_by_name::<Setting>("School"), None);
        assert_eq!(context.setting_name(COMMUNITY), "Community");
    }

    #[test]
    fn multiplier_follows_alpha() {
        let (mut context, people) = context_with(4);
        home_for_all(&mut context, &people);
        let work = context.create_setting_group(WORK);
        for &p in &people {
            context.add_setting_member(work, p);
        }
        let community = context.create_population_setting_group(COMMUNITY);
        assert_eq!(context.setting_group(people[0], COMMUNITY), Some(community));
        assert_eq!(context.setting_group_size(people[0], COMMUNITY), 4);
        for (setting, expected) in [(HOME, 3.0), (WORK, 3f64.sqrt()), (COMMUNITY, 1.0)] {
            context.set_itinerary(people[0], Itinerary::new([(setting, 1.0)]));
            assert_eq!(
                context.current_setting_multiplier::<Setting>(people[0]),
                expected
            );
            // The bound covers every kind, whatever the itinerary.
            assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 3.0);
        }
    }

    #[test]
    fn current_multiplier_is_the_itinerary_weighted_average() {
        let (mut context, people) = context_with(3);
        home_for_all(&mut context, &people);
        context.create_population_setting_group(COMMUNITY);
        context.set_itinerary_assignment(Itinerary::new([(HOME, 0.25), (COMMUNITY, 0.75)]));
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
        let home = context.create_setting_group(HOME);
        context.add_setting_member(home, people[0]);
        context.set_itinerary_assignment(Itinerary::new([(HOME, 1.0)]));
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[1]),
            0.0
        );
        assert_eq!(context.sample_setting::<Setting>(people[1]), None);
        assert_eq!(context.sample_setting_contact(people[1], HOME), None);
        // Alone in the group.
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[0]),
            0.0
        );
        assert_eq!(context.sample_setting_contact(people[0], HOME), None);
    }

    #[test]
    fn moving_groups_updates_both_member_lists() {
        let (mut context, people) = context_with(3);
        let a = home_for_all(&mut context, &people);
        let b = context.create_setting_group(HOME);
        context.add_setting_member(b, people[2]);
        context.add_setting_member(b, people[2]);
        assert_eq!(context.setting_group(people[2], HOME), Some(b));
        assert_eq!(context.setting_group(people[0], HOME), Some(a));
        assert_eq!(context.setting_group_size(people[0], HOME), 2);
        assert_eq!(context.setting_group_size(people[2], HOME), 1);
        for _ in 0..20 {
            let contact = context.sample_setting_contact(people[0], HOME).unwrap();
            assert_eq!(contact, people[1]);
        }
    }

    #[test]
    fn removing_members_lowers_the_current_but_not_the_bound() {
        let (mut context, people) = context_with(3);
        let home = home_for_all(&mut context, &people);
        context.set_itinerary_assignment(Itinerary::new([(HOME, 1.0)]));
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 2.0);

        context.remove_setting_member(people[1], HOME);
        assert_eq!(context.setting_group(people[1], HOME), None);
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[0]),
            1.0
        );
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 2.0);
        assert_eq!(
            context.sample_setting_contact(people[0], HOME),
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
        let (mut context, people) = context_with_work_alpha(3, 1.0);
        let home = home_for_all(&mut context, &people);
        context.remove_setting_member(people[1], HOME);
        context.remove_setting_member(people[1], HOME);
        assert_eq!(context.setting_group(people[1], HOME), None);
        assert_eq!(context.setting_group_size(people[0], HOME), 2);
        context.restore_setting_member(people[1], HOME);
        assert_eq!(context.setting_group(people[1], HOME), Some(home));
        assert_eq!(context.setting_group_size(people[0], HOME), 3);
        // Never a member: nothing to restore.
        context.restore_setting_member(people[1], WORK);
        assert_eq!(context.setting_group(people[1], WORK), None);
    }

    #[test]
    fn itinerary_renormalizes_over_the_kinds_a_person_has() {
        let (mut context, people) = context_with_work_alpha(3, 1.0);
        home_for_all(&mut context, &people[..2]);
        let work = context.create_setting_group(WORK);
        for &p in &people {
            context.add_setting_member(work, p);
        }
        context.set_itinerary_assignment(Itinerary::new([(HOME, 0.5), (WORK, 0.5)]));
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
        context.remove_setting_member(people[0], WORK);
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[0]),
            1.0
        );
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 2.0);
        for _ in 0..20 {
            assert_eq!(context.sample_setting::<Setting>(people[0]), Some(HOME));
        }
    }

    #[test]
    fn population_group_samples_anyone_else() {
        let (mut context, people) = context_with(3);
        context.create_population_setting_group(COMMUNITY);
        for _ in 0..20 {
            let contact = context
                .sample_setting_contact(people[0], COMMUNITY)
                .unwrap();
            assert_ne!(contact, people[0]);
        }
    }

    #[test]
    #[should_panic(expected = "population group")]
    fn cannot_leave_a_population_group() {
        let (mut context, people) = context_with(1);
        context.create_population_setting_group(COMMUNITY);
        context.remove_setting_member(people[0], COMMUNITY);
    }

    #[test]
    fn itinerary_normalizes() {
        let itinerary = Itinerary::new([(HOME, 2.0), (COMMUNITY, 6.0)]);
        assert_eq!(itinerary.ratio(HOME), 0.25);
        assert_eq!(itinerary.ratio(WORK), 0.0);
        assert_eq!(itinerary.ratio(COMMUNITY), 0.75);
    }

    #[test]
    fn draw_setting_by_weight() {
        let mut weights = [0.0; MAX_SETTINGS];
        assert_eq!(draw_setting::<Setting>(&weights, 0.5), None);
        weights[HOME.index()] = 1.0;
        weights[COMMUNITY.index()] = 3.0;
        assert_eq!(draw_setting::<Setting>(&weights, 0.0), Some(HOME));
        assert_eq!(draw_setting::<Setting>(&weights, 0.2499), Some(HOME));
        assert_eq!(draw_setting::<Setting>(&weights, 0.25), Some(COMMUNITY));
        assert_eq!(draw_setting::<Setting>(&weights, 0.9999), Some(COMMUNITY));
    }

    #[test]
    fn sample_setting_draws_by_ratio_over_kinds_with_a_group() {
        let (mut context, people) = context_with_work_alpha(1, 1.0);
        home_for_all(&mut context, &people);
        context.set_itinerary_assignment(Itinerary::new([(HOME, 0.1), (WORK, 0.9)]));
        // No workplace, so every attempt is at home, even alone there: the
        // multiplier, not the draw, thins it out.
        for _ in 0..20 {
            assert_eq!(context.sample_setting(people[0]), Some(HOME));
        }
        assert_eq!(context.setting_multiplier(people[0], HOME), 0.0);
        assert_eq!(context.setting_multiplier(people[0], WORK), 0.0);
    }

    #[test]
    fn personal_itinerary_overrides_the_assignment() {
        let (mut context, people) = context_with(3);
        home_for_all(&mut context, &people);
        context.create_population_setting_group(COMMUNITY);
        assert_eq!(context.sample_setting::<Setting>(people[0]), None);
        // The bound does not wait for an itinerary.
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 2.0);

        context.set_itinerary_assignment(Itinerary::new([(COMMUNITY, 1.0)]));
        context.set_itinerary(people[1], Itinerary::new([(HOME, 1.0)]));
        assert_eq!(context.sample_setting(people[0]), Some(COMMUNITY));
        assert_eq!(context.sample_setting(people[1]), Some(HOME));
        // Both are members at home, so both bounds allow for it.
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), 2.0);
        assert_eq!(context.max_setting_multiplier::<Setting>(people[1]), 2.0);
    }

    #[test]
    fn bound_holds_across_an_itinerary_override_and_its_removal() {
        // Alone at home (multiplier 0), with two others at work (multiplier 2).
        let (mut context, people) = context_with_work_alpha(3, 1.0);
        home_for_all(&mut context, &people[..1]);
        let work = context.create_setting_group(WORK);
        for &p in &people {
            context.add_setting_member(work, p);
        }
        context.set_itinerary_assignment(Itinerary::new([(HOME, 0.5), (WORK, 0.5)]));

        // Isolated at home: a bound from the current itinerary alone would be
        // 0, ending the transmission loop before isolation does.
        context.set_itinerary(people[0], Itinerary::new([(HOME, 1.0)]));
        let bound = context.max_setting_multiplier::<Setting>(people[0]);
        assert_eq!(bound, 2.0);

        context.clear_itinerary::<Setting>(people[0]);
        for _ in 0..50 {
            let setting = context.sample_setting::<Setting>(people[0]).unwrap();
            assert!(context.setting_multiplier(people[0], setting) <= bound);
        }
        assert_eq!(context.max_setting_multiplier::<Setting>(people[0]), bound);
    }

    #[test]
    fn itinerary_assigned_by_property_follows_the_current_value() {
        let (mut context, people) = context_with(2);
        home_for_all(&mut context, &people);
        context.create_population_setting_group(COMMUNITY);
        let child = Itinerary::new([(HOME, 1.0)]);
        let adult = Itinerary::new([(COMMUNITY, 1.0)]);
        context.set_itinerary_assignment(ItineraryAssignment::by_property(move |age: AgeGroup| {
            match age {
                AgeGroup::Child => child,
                AgeGroup::Adult => adult,
            }
        }));
        context.set_property(people[0], AgeGroup::Child);
        assert_eq!(context.sample_setting(people[0]), Some(HOME));
        assert_eq!(context.sample_setting(people[1]), Some(COMMUNITY));
        context.set_property(people[0], AgeGroup::Adult);
        assert_eq!(context.sample_setting(people[0]), Some(COMMUNITY));
    }

    #[test]
    fn itinerary_assigned_individually_by_closure() {
        let (mut context, people) = context_with(2);
        context.create_population_setting_group(COMMUNITY);
        context.set_itinerary_assignment(|_: &Context, _| Itinerary::new([(COMMUNITY, 1.0)]));
        assert_eq!(context.sample_setting(people[1]), Some(COMMUNITY));
    }

    #[test]
    fn contact_needs_someone_else_in_the_setting() {
        let (mut context, people) = context_with_work_alpha(3, 1.0);
        home_for_all(&mut context, &people);
        let work = context.create_setting_group(WORK);
        context.add_setting_member(work, people[0]);
        context.set_itinerary_assignment(Itinerary::new([(HOME, 0.5), (WORK, 0.5)]));
        // Alone at work: no contact there, and the multiplier is zero.
        assert_eq!(context.setting_multiplier(people[0], WORK), 0.0);
        assert_eq!(context.sample_setting_contact(people[0], WORK), None);
        assert_eq!(context.setting_multiplier(people[0], HOME), 2.0);
        for _ in 0..20 {
            let contact = context.sample_setting_contact(people[0], HOME).unwrap();
            assert_ne!(contact, people[0]);
        }
        context.remove_setting_member(people[0], HOME);
        assert_eq!(context.sample_setting(people[0]), Some(WORK));
        assert_eq!(context.setting_multiplier(people[0], HOME), 0.0);
    }

    #[test]
    fn groups_by_key_are_created_once_per_kind() {
        let (mut context, _) = context_with(0);
        let a = context.setting_group_for_key(HOME, 7u64);
        let b = context.setting_group_for_key(HOME, 8u64);
        assert_ne!(a, b);
        assert_eq!(context.setting_group_for_key(HOME, 7u64), a);
        assert_ne!(context.setting_group_for_key(WORK, 7u64).index, a.index);
        assert_eq!(context.setting_group_by_key(HOME, 8u64), Some(b));
        assert_eq!(context.setting_group_by_key(HOME, 9u64), None);
        // Owned and borrowed strings are the same key.
        let s = context.setting_group_for_key(HOME, "HH1".to_string());
        assert_eq!(context.setting_group_by_key(HOME, "HH1"), Some(s));
    }

    #[test]
    fn a_kind_added_after_members_exist_widens_their_records() {
        // Memberships in all three kinds before the records are widened.
        let (mut context, people) = context_with_work_alpha(4, 1.0);
        let home = home_for_all(&mut context, &people[..3]);
        let work = context.create_setting_group(WORK);
        for &person in &people[1..] {
            context.add_setting_member(work, person);
        }
        context.create_population_setting_group(COMMUNITY);
        context.remove_setting_member(people[2], WORK);

        let school = context.add_setting_kind::<Setting>("School", 0.0);
        let class = context.create_setting_group(school);
        context.add_setting_member(class, people[0]);
        context.add_setting_member(class, people[3]);

        for &person in &people[..3] {
            assert_eq!(context.setting_group(person, HOME), Some(home));
            assert_eq!(context.setting_multiplier(person, HOME), 2.0);
        }
        assert_eq!(context.setting_group(people[3], HOME), None);
        assert_eq!(context.setting_group(people[0], WORK), None);
        assert_eq!(context.setting_group(people[1], WORK), Some(work));
        assert_eq!(context.setting_group(people[2], WORK), None);
        assert_eq!(context.setting_group_size(people[0], COMMUNITY), 4);
        assert_eq!(context.setting_group(people[1], school), None);
        assert_eq!(context.setting_group_size(people[3], school), 2);
        assert_eq!(
            context.sample_setting_contact(people[3], school),
            Some(people[0])
        );

        // Positions still line up: swap-remove, then restore, after widening.
        context.remove_setting_member(people[1], WORK);
        assert_eq!(context.setting_group_members(work), vec![people[3]]);
        context.restore_setting_member(people[2], WORK);
        context.restore_setting_member(people[1], WORK);
        assert_eq!(
            context.setting_group_members(work),
            vec![people[3], people[2], people[1]]
        );
        assert_eq!(context.setting_multiplier(people[1], WORK), 2.0);
    }

    #[test]
    #[should_panic(expected = "already exists")]
    fn kind_names_are_unique() {
        let (mut context, _) = context_with(0);
        context.add_setting_kind::<Setting>("Home", 0.0);
    }

    #[test]
    #[should_panic(expected = "at most")]
    fn kinds_are_capped() {
        let mut context = Context::new();
        for i in 0..=MAX_SETTINGS {
            context.add_setting_kind::<Setting>(&format!("kind {i}"), 1.0);
        }
    }

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn sample_setting_by_rate_weights_the_ratio_by_the_multiplier() {
        // Home: 3 people, alpha 1, multiplier 2. Work: 5 people, alpha 1,
        // multiplier 4. Ratios 0.6 / 0.4 give rates 1.2 / 1.6.
        let (mut context, people) = context_with_work_alpha(5, 1.0);
        home_for_all(&mut context, &people[..3]);
        let work = context.create_setting_group(WORK);
        for &p in &people {
            context.add_setting_member(work, p);
        }
        context.set_itinerary_assignment(Itinerary::new([(HOME, 0.6), (WORK, 0.4)]));
        let total = context.current_setting_multiplier::<Setting>(people[0]);
        assert!((total - 2.8).abs() < 1e-12);
        assert!(total <= context.max_setting_multiplier::<Setting>(people[0]));

        let draws = 20_000;
        let mut at_home = 0;
        for _ in 0..draws {
            if context.sample_setting_by_rate::<Setting>(people[0]) == Some(HOME) {
                at_home += 1;
            }
        }
        let observed = f64::from(at_home) / f64::from(draws);
        assert!(
            (observed - 1.2 / 2.8).abs() < 0.02,
            "expected about {}, observed {observed}",
            1.2 / 2.8
        );

        // Someone with no group at home is always drawn at work.
        assert_eq!(
            context.sample_setting_by_rate::<Setting>(people[4]),
            Some(WORK)
        );
    }

    #[test]
    fn sample_setting_by_rate_is_none_when_no_setting_has_a_rate() {
        // Alone at home: a ratio but a multiplier of 0.
        let (mut context, people) = context_with(1);
        home_for_all(&mut context, &people);
        assert_eq!(context.sample_setting_by_rate::<Setting>(people[0]), None);
        context.set_itinerary_assignment(Itinerary::new([(HOME, 1.0)]));
        assert_eq!(
            context.current_setting_multiplier::<Setting>(people[0]),
            0.0
        );
        assert_eq!(context.sample_setting_by_rate::<Setting>(people[0]), None);
    }

    #[test]
    fn group_members_lists_who_is_in_a_group_now() {
        let (mut context, people) = context_with(4);
        let home = home_for_all(&mut context, &people[..3]);
        context.remove_setting_member(people[1], HOME);
        assert_eq!(
            context.setting_group_members(home),
            vec![people[0], people[2]]
        );
        let community = context.create_population_setting_group(COMMUNITY);
        assert_eq!(context.setting_group_members(community), people);
    }

    #[test]
    fn moving_back_to_the_population_group_forgets_the_list_group() {
        let (mut context, people) = context_with(3);
        let everyone = context.create_population_setting_group(COMMUNITY);
        let bubble = context.create_setting_group(COMMUNITY);
        context.add_setting_member(bubble, people[0]);
        assert_eq!(context.setting_group(people[0], COMMUNITY), Some(bubble));

        context.add_setting_member(everyone, people[0]);
        assert_eq!(context.setting_group(people[0], COMMUNITY), Some(everyone));
        assert_eq!(context.setting_group_size(people[0], COMMUNITY), 3);
        assert!(context.setting_group_members(bubble).is_empty());
        // Nothing left to restore them to.
        context.restore_setting_member(people[0], COMMUNITY);
        assert_eq!(context.setting_group(people[0], COMMUNITY), Some(everyone));
    }

    #[test]
    fn someone_alone_in_a_group_has_no_multiplier_whatever_alpha() {
        let (mut context, people) = context_with(1);
        let alone = context.create_setting_group(COMMUNITY);
        context.add_setting_member(alone, people[0]);
        assert_eq!(context.setting_multiplier(people[0], COMMUNITY), 0.0);
        home_for_all(&mut context, &people);
        assert_eq!(context.setting_multiplier(people[0], HOME), 0.0);
    }
}
