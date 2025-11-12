/*!
`ValueVec<T>`: a by-value, ref-less vector with interior mutability.

You can convert to and from a `Vec<T>` at zero cost.

Key properties:
- All mutating operations use `&self` (immutable receiver).
- No references to elements are ever given out.
- Elements are inserted/removed/moved **by value**.
- Getting a value returns a `Clone` (or `Copy`) of the stored element.
- Many shared immutable references to a `ValueVec` can exist simultaneously safely.

Trade-offs:
- You can’t borrow into the backing memory; you pay cloning cost to read.
- In return, the backing storage may reallocate at will without invalidating
  any external references (because none are ever issued).

Functionality:

For any functionality of `Vec<T>` that `ValueVec<T>` doesn't provide,
you can use `into_vec` to convert to a `Vec<T>` at zero cost.

Soundness:

We require `T` to be `Copy` to avoid subtle soundness issues. However, this requirement
could be replaced with a requirement that prevents re-entrance into methods of
`ValueVec<T>` (directly or indirectly) from the `Drop` or `Clone` implementations of `T`.

*/

use std::cell::UnsafeCell;
use std::fmt::Debug;

/**
A by-value, `ref`-less vector with interior mutability. Values of type `V` can be moved into and out of the vector. We require `V` to be `Copy` to avoid subtle soundness issues.
*/
pub struct ValueVec<V: Copy> {
    data: UnsafeCell<Vec<V>>,
}

impl<V: Copy> ValueVec<V> {
    /// Creates an empty `ValueVec`.
    pub fn new() -> Self {
        Self {
            data: UnsafeCell::new(Vec::new()),
        }
    }

    /// Creates with capacity.
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            data: UnsafeCell::new(Vec::with_capacity(cap)),
        }
    }

    /// Current number of elements.
    #[inline]
    pub fn len(&self) -> usize {
        // Safety: This is already an immutable operation
        unsafe { (&*self.data.get()).len() }
    }

    /// Current capacity of the backing Vec.
    #[inline]
    pub fn capacity(&self) -> usize {
        // Safety: This is already an immutable operation
        unsafe { (&*self.data.get()).capacity() }
    }

    /// Returns true if the vector has no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Ensures capacity for at least `additional` more elements.
    pub fn reserve(&self, additional: usize) {
        self.with_vec(|v| v.reserve(additional));
    }

    /// Shrinks the capacity as much as possible.
    pub fn shrink_to_fit(&self) {
        self.with_vec(|v| v.shrink_to_fit());
    }

    /// Pushes a value (by move) onto the end.
    pub fn push(&self, value: V) {
        self.with_vec(|v| v.push(value));
    }

    /// Pops and **returns** the last element (by move), or `None` if empty.
    pub fn pop(&self) -> Option<V> {
        self.with_vec(|v| v.pop())
    }

    /// Returns the value of the element at `index`, if `index` is in range. Returns `None` if `index` is out of bounds.
    /// This is a bounds-checked variant of [`ValueVec::at`].
    pub fn get(&self, index: usize) -> Option<V> {
        unsafe { (&*self.data.get()).get(index).copied() }
    }

    /// Returns the value at `index`. Panics if `index` is out of bounds.
    ///
    /// Use [`ValueVec::get`] for a bounds-checked version of this method.
    pub fn at(&self, index: usize) -> V {
        unsafe { (&*self.data.get())[index] }
    }

    /// Moves a value into the slot at `index`, returning the old value (via move). Panics if `index` is out of bounds.
    pub fn replace(&self, index: usize, value: V) -> V {
        self.with_vec(|v| core::mem::replace(&mut v[index], value))
    }

    /// Sets the value of the slot at `index` to `value`. Panics if `index` is out of bounds.
    pub fn set(&self, index: usize, value: V) {
        self.with_vec(|v| v[index] = value)
    }

    /// Swaps the value at `index` with the provided one in place. Panics if `index` is out of bounds.
    ///
    /// The existing value ends up in `*value`.
    pub fn swap_value(&self, index: usize, value: &mut V) {
        self.with_vec(|v| {
            core::mem::swap(&mut v[index], value);
        })
    }

    /// Inserts `value` at position `index`, shifting elements to the right. Panics if `index` is out of bounds.
    pub fn insert(&self, index: usize, value: V) {
        self.with_vec(|v| {
            v.insert(index, value);
        })
    }

    /// Removes and returns the element at `index`, shifting elements left. Panics if `index` is out of bounds.
    pub fn remove(&self, index: usize) -> V {
        self.with_vec(|v| v.remove(index))
    }

    /// Removes and returns the element at `index` by swapping in the last element.
    ///
    /// O(1) removal when order does not matter.
    pub fn swap_remove(&self, index: usize) -> V {
        self.with_vec(|v| v.swap_remove(index))
    }

    /// Returns `true` if the `ValueVec` contains an element with the given value.
    pub fn contains(&self, value: &V) -> bool
    where
        V: PartialEq,
    {
        self.with_vec(|v| v.contains(value))
    }

    /// Clears all elements.
    pub fn clear(&self) {
        self.with_vec(|v| v.clear());
    }

    /// Extends the vector by moving in elements from an iterator.
    pub fn extend<I>(&self, iter: I)
    where
        I: IntoIterator<Item = V>,
    {
        self.with_vec(|v| v.extend(iter));
    }

    pub fn resize(&self, new_len: usize, value: V) {
        self.with_vec(|v| v.resize(new_len, value));
    }

    pub fn resize_with<F>(&self, new_len: usize, f: F)
    where
        F: FnMut() -> V,
    {
        self.with_vec(|v| v.resize_with(new_len, f));
    }

    /// Returns a **snapshot** `Vec<V>` by cloning all elements.
    ///
    /// Use `From<ValueVec<V>> for Vec<V>` for a zero-cost conversion if you don't want to clone.
    pub fn to_vec(&self) -> Vec<V>
    where
        V: Clone,
    {
        unsafe { (&*self.data.get()).clone() }
    }

    /// Applies `f` with exclusive access to the inner Vec.
    ///
    /// **Safety:** `with_vec` temporarily obtains an exclusive `&mut Vec<V>`
    /// from the internal `UnsafeCell` and passes it to the provided closure.
    /// This is considered sound **only under controlled internal use**:
    ///
    /// - The mutable borrow does not escape the function.
    /// - No references to elements are ever returned or stored; all access
    ///   to elements occurs by value (move or clone).
    /// - Only one mutable borrow of the internal `Vec` exists at a time,
    ///   and it ends before the method returns.
    ///
    /// The one additional requirement for soundness is that the closure
    /// passed to `with_vec` must not re-enter any other `ValueVec` method
    /// (directly or indirectly) while it holds the mutable borrow. Such
    /// re-entrancy could occur, for example, from a user-defined `Drop` or
    /// `Clone` implementation of `V` that calls back into the same instance,
    /// and would cause overlapping borrows of the internal `Vec`, leading to
    /// undefined behavior.
    ///
    /// This function remains private to ensure that every closure passed to
    /// it is under our control and has been manually verified to uphold the
    /// no-reentrancy invariant. Its soundness depends on that internal
    /// discipline.
    #[inline]
    fn with_vec<R>(&self, f: impl FnOnce(&mut Vec<V>) -> R) -> R {
        // SAFETY: `UnsafeCell` permits obtaining a unique mutable reference here.
        // We never let any references escape, and we serialize access by construction
        // (each method call performs one short-lived exclusive borrow of the Vec).
        let vec: &mut Vec<V> = unsafe { &mut *self.data.get() };
        f(vec)
    }
}

impl<V: Copy> Default for ValueVec<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: Copy + Debug> Debug for ValueVec<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // SAFETY: We create a temporary shared reference to the inner Vec.
        // No mutable borrows of the Vec exist concurrently by design.
        let vec = unsafe { &*self.data.get() };
        vec.fmt(f)
    }
}

impl<V: Copy> From<Vec<V>> for ValueVec<V> {
    /// Wraps an existing `Vec` without copying its elements.
    fn from(src: Vec<V>) -> Self {
        Self {
            data: UnsafeCell::new(src),
        }
    }
}

impl<V: Copy> From<ValueVec<V>> for Vec<V> {
    fn from(val: ValueVec<V>) -> Self {
        val.data.into_inner()
    }
}

impl<V: Copy> IntoIterator for ValueVec<V> {
    type Item = V;
    type IntoIter = std::vec::IntoIter<V>;

    fn into_iter(self) -> Self::IntoIter {
        // SAFETY: We are consuming `self`, so there can be no remaining references
        // to the inner Vec. It is therefore safe to move it out of the UnsafeCell.
        let vec = self.data.into_inner();
        vec.into_iter()
    }
}

impl<V: Copy> FromIterator<V> for ValueVec<V> {
    fn from_iter<I: IntoIterator<Item = V>>(iter: I) -> Self {
        Self::from(Vec::from_iter(iter))
    }
}

#[cfg(test)]
mod tests {
    use super::ValueVec;

    #[test]
    fn push_pop() {
        let v = ValueVec::new();
        assert!(v.is_empty());
        v.push(1);
        v.push(2);
        v.push(3);
        assert_eq!(v.len(), 3);
        assert_eq!(v.pop(), Some(3));
        assert_eq!(v.pop(), Some(2));
        assert_eq!(v.pop(), Some(1));
        assert_eq!(v.pop(), None);
    }

    #[test]
    fn get_cloned_and_replace() {
        let v = ValueVec::new();
        v.extend([10, 20, 30]);
        assert_eq!(v.get(1), Some(20));
        assert_eq!(v.replace(1, 99), 20);
        assert_eq!(v.get(1), Some(99));
    }

    #[test]
    fn insert_and_remove() {
        let v = ValueVec::new();
        v.extend([1, 2, 3]);
        v.insert(1, 9); // [1, 9, 2, 3]
        assert_eq!(v.len(), 4);
        assert_eq!(v.remove(2), 2); // [1, 9, 3]
        assert_eq!(v.get(0), Some(1));
        assert_eq!(v.at(1), 9);
        assert_eq!(v.at(2), 3);
        assert_eq!(v.remove(1), 9);
    }

    #[test]
    fn swap_remove_works() {
        let v = ValueVec::new();
        v.extend([10, 20, 30, 40]);
        let got = v.swap_remove(1);
        assert_eq!(got, 20);
        // Order not guaranteed after swap_remove, but len decreased:
        assert_eq!(v.len(), 3);
        let snapshot = v.to_vec();
        assert!(snapshot.contains(&10));
        assert!(snapshot.contains(&30));
        assert!(snapshot.contains(&40));
        assert!(!snapshot.contains(&20));

        let v: Vec<i32> = v.into();
        assert_eq!(snapshot, v);
    }

    #[test]
    fn to_vec_clone_snapshot() {
        let v = ValueVec::new();
        v.extend(["a", "b"]);
        let snap = v.to_vec();
        assert_eq!(snap, vec!["a", "b"]);
        // Mutate original; snapshot unaffected.
        v.push("c");
        assert_eq!(v.len(), 3);
        assert_eq!(snap.len(), 2);
    }

    #[test]
    fn debug_impl() {
        let v = ValueVec::from(vec![1, 2, 3]);
        let s = format!("{v:?}");
        println!("{}", s);
        assert!(!s.contains("ValueVec"));
        assert!(s.contains("[1, 2, 3]"));
    }

    #[test]
    fn from_vec_wraps_without_copy() {
        let v = vec![1, 2, 3];
        let vv = ValueVec::from(v);
        // We can't check memory identity directly, but we can check content and len.
        assert_eq!(vv.len(), 3);
        assert_eq!(vv.at(0), 1);
        assert_eq!(vv.at(1), 2);
        assert_eq!(vv.at(2), 3);
        // Original vector is consumed and cannot be used.
    }

    #[test]
    fn into_vec_unwraps_without_copy() {
        let vv = ValueVec::from(vec!["a", "b"]);
        let v: Vec<_> = vv.into(); // moves out inner Vec
        assert_eq!(v, vec!["a", "b"]);
    }

    #[test]
    fn round_trip_from_vec_into_vec() {
        let original = vec![10, 20, 30];
        let vv = ValueVec::from(original);
        let roundtrip: Vec<_> = vv.into();
        assert_eq!(roundtrip, vec![10, 20, 30]);
    }

    #[test]
    fn into_iterator_consumes_valuevec() {
        let vv = ValueVec::from(vec![1, 2, 3]);
        let collected: Vec<_> = vv.into_iter().collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }

    #[test]
    fn into_iterator_and_into_vec_equivalence() {
        let vv1 = ValueVec::from(vec![5, 6, 7]);
        let vv2 = ValueVec::from(vec![5, 6, 7]);
        let collected_from_iter: Vec<_> = vv1.into_iter().collect();
        let collected_from_into: Vec<_> = vv2.into();
        assert_eq!(collected_from_iter, collected_from_into);
    }

    #[test]
    fn from_iterator_collect() {
        let source = [1, 2, 3, 4, 5];
        let vv: ValueVec<i32> = source.iter().copied().collect();
        assert_eq!(vv.len(), 5);
        assert_eq!(vv.at(0), 1);
        assert_eq!(vv.at(4), 5);
    }
}
