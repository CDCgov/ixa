use crate::HashMap;
use std::{
    any::{Any, TypeId},
    cell::OnceCell,
    marker::PhantomData,
    sync::{atomic::AtomicUsize, LazyLock, Mutex},
};

use anyhow::{anyhow, Result};

#[allow(dead_code)]
pub static TYPE_STORE_INDEXER: LazyLock<Mutex<HashMap<TypeId, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

pub static TYPE_STORE_SIZES: LazyLock<Mutex<HashMap<TypeId, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

pub trait TypeIndex: 'static {
    type Category: TypeIndexCategory;
    type Data: Any;
    fn type_index() -> usize;
}

pub trait TypeIndexCategory {
    fn register_type();
    fn size() -> usize;
}

pub struct TypeStore<C: TypeIndexCategory> {
    _marker: PhantomData<C>,
    store: Vec<OnceCell<Box<dyn Any>>>,
}

impl<C: TypeIndexCategory> TypeStore<C> {
    pub fn new() -> Self {
        let size = C::size();
        Self {
            _marker: PhantomData,
            store: std::iter::repeat_with(OnceCell::new).take(size).collect(),
        }
    }
    pub fn add<T: TypeIndex<Category = C> + Any>(&mut self, item: T) -> Result<()> {
        let index = T::type_index();
        let cell: OnceCell<Box<dyn Any>> = OnceCell::new();
        cell.set(Box::new(item))
            .map_err(|_| anyhow!("There was an existing value"))?;
        self.store[index] = cell;
        Ok(())
    }
    pub fn get<T: TypeIndex<Category = C> + Any>(&self) -> Option<&T::Data> {
        let index = T::type_index();
        self.store.get(index)?.get()?.downcast_ref()
    }
    pub fn get_or_init<T: TypeIndex<Category = C> + Any, I: FnOnce() -> T::Data>(
        &self,
        initializer: I,
    ) -> &T::Data {
        let index = T::type_index();
        self.get_by_index::<T::Data, I>(index, initializer)
            .expect("Could not cast to the right type")
    }
    pub fn get_by_index<T: Any, I: FnOnce() -> T>(
        &self,
        index: usize,
        initializer: I,
    ) -> Option<&T> {
        self.store
            .get(index)
            .expect("Index out of range")
            .get_or_init(|| Box::new(initializer()))
            .downcast_ref()
    }
    pub fn get_mut<T: TypeIndex<Category = C> + Any>(&mut self) -> Option<&mut T::Data> {
        let index = T::type_index();
        self.get_mut_by_index(index)
    }
    pub fn get_mut_by_index<T: Any>(&mut self, index: usize) -> Option<&mut T> {
        self.store
            .get_mut(index)?
            .get_mut()
            .unwrap()
            .downcast_mut::<T>()
    }
    pub fn get_or_init_mut<T: TypeIndex<Category = C> + Any, I: FnOnce() -> T::Data>(
        &mut self,
        initializer: I,
    ) -> &mut T::Data {
        let index = T::type_index();
        self.get_or_init_mut_by_index::<T::Data, I>(index, initializer)
            .expect("Could not cast to the right type")
    }
    pub fn get_or_init_mut_by_index<T: Any, I: FnOnce() -> T>(
        &mut self,
        index: usize,
        initializer: I,
    ) -> Option<&mut T> {
        let cell = self.store.get_mut(index).expect("Index out of range");

        if cell.get().is_none() {
            cell.set(Box::new(initializer()))
                .map_err(|_| "Cell was already initialized")
                .unwrap();
        }

        cell.get_mut().unwrap().downcast_mut::<T>()
    }
}

impl<C: TypeIndexCategory> Default for TypeStore<C> {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
pub fn init_type_index(category_type_id: TypeId, index_holder: &AtomicUsize) -> usize {
    // Fast path: already initialized.
    let index = index_holder.load(std::sync::atomic::Ordering::Relaxed);
    if index != usize::MAX {
        return index;
    }

    let mut map = TYPE_STORE_INDEXER.lock().unwrap();

    let candidate_ref = map.entry(category_type_id).or_insert_with(|| 0);
    let candidate = *candidate_ref;

    // Try to claim the candidate index. Here we guard against the potential race condition that
    // another instance of this plugin in another thread just initialized the index prior to us
    // obtaining the lock. If the index has been initialized beneath us, we do not update
    // `NEXT_DATA_PLUGIN_INDEX`, we just return the value `plugin_index` was initialized to.
    // For a justification of the data ordering, see:
    //     https://github.com/CDCgov/ixa/pull/477#discussion_r2244302872
    match index_holder.compare_exchange(
        usize::MAX,
        candidate,
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
    ) {
        Ok(_) => {
            // We won the race — increment the global next plugin index and return the new index
            *candidate_ref += 1;
            candidate
        }
        Err(existing) => {
            // Another thread beat us — don’t increment the global next plugin index,
            // just return existing
            existing
        }
    }
}

#[macro_export]
macro_rules! define_type_store {
    ($store: ident<$category: ident$(, $data: ty)?>) => {
        $crate::paste::paste! {
            pub struct $category;

            impl $crate::type_store::TypeIndexCategory for $category {
                fn register_type() {
                    let category_type_id = std::any::TypeId::of::<Self>();
                    let mut map = $crate::type_store::TYPE_STORE_SIZES.lock().unwrap();
                    let entry = map.entry(category_type_id).or_insert_with(|| 0);
                    *entry += 1;
                }
                fn size() -> usize {
                    let category_type_id = std::any::TypeId::of::<Self>();
                    let map = $crate::type_store::TYPE_STORE_SIZES.lock().unwrap();
                    map.get(&category_type_id).map(|v| *v).unwrap_or(0)
                }
            }

            #[allow(dead_code)]
            pub type $store = $crate::type_store::TypeStore<$category>;
        }
    };
}

#[macro_export]
macro_rules! type_index {
    ($key:ty, $category:ty) => {
        $crate::type_index!($key, $key, $category);
    };
    ($key:ty, $data:ty, $category: ty) => {
        $crate::paste::paste! {
            #[$crate::ctor::ctor]
            fn [<_register_type_index_$key:snake>]() {
                <$category as $crate::type_store::TypeIndexCategory>::register_type();
            }

            impl $crate::type_store::TypeIndex for $key {
                type Category = $category;
                type Data = $data;

                fn type_index() -> usize {
                    // This static must be initialized with a compile-time constant expression.
                    // We use `usize::MAX` as a sentinel to mean "uninitialized". This
                    // static variable is shared among all instances of this data plugin type.
                    static INDEX: std::sync::atomic::AtomicUsize =
                        std::sync::atomic::AtomicUsize::new(usize::MAX);

                    // Slow path: initialize it.
                    let category_type_id = std::any::TypeId::of::<Self::Category>();
                    $crate::type_store::init_type_index(category_type_id, &INDEX)

                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct A1(usize);

    #[derive(Default)]
    struct A2(usize);

    #[derive(Default)]
    struct B1(usize);

    #[derive(Default)]
    struct B2(usize);

    define_type_store!(AStore<APlugin>);
    define_type_store!(BStore<BPlugin>);

    type_index!(A1, APlugin);
    type_index!(A2, APlugin);

    type_index!(B1, BPlugin);
    type_index!(B2, BPlugin);

    #[test]
    fn test_type_index() {
        assert_ne!(A1::type_index(), A2::type_index());
        assert_ne!(B1::type_index(), B2::type_index());
    }

    #[test]
    fn test_store() {
        let mut a_store = AStore::new();
        let mut b_store = BStore::new();
        let a1 = A1(1);
        let a2 = A2(2);
        let b1 = B1(1);
        let b2 = B2(2);

        dbg!(a1.0);

        a_store.add(a1).unwrap();
        a_store.add(a2).unwrap();

        b_store.add(b1).unwrap();
        b_store.add(b2).unwrap();

        assert_eq!(a_store.get::<A1>().unwrap().0, 1);
        assert_eq!(a_store.get::<A2>().unwrap().0, 2);
        assert_eq!(b_store.get::<B1>().unwrap().0, 1);
        assert_eq!(b_store.get::<B2>().unwrap().0, 2);
    }
}
