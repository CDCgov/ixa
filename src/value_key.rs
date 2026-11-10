//! Equality and hashing of property values: support code for `EqHash`, not public API.
//!
//! Property types get `PartialEq`, `Eq` and `Hash` from [`EqHash`](crate::EqHash), which
//! compares and hashes each field by its [`ValueKey`] when the field type has one (that takes
//! precedence over a type's own `Eq` and `Hash`), and by the field's own `Eq` and `Hash`
//! otherwise. Floats have a `ValueKey`: their bit pattern with
//! `-0.0` folded into `0.0` and every `NaN` folded into one, so a float field compares like
//! IEEE equality except that `NaN` equals itself, which an index or hash map needs.

use std::hash::Hash;
use std::marker::PhantomData;

/// A value compared and hashed through a key rather than through its own `Eq` and `Hash`.
///
/// Implemented for `f32` and `f64`, for the integer, `bool`, `char` and `()` types as the
/// identity, and for `Option`s, arrays and tuples of up to eight implementors.
pub trait ValueKey {
    type Key: Eq + Hash;

    fn value_key(&self) -> Self::Key;
}

impl ValueKey for f64 {
    type Key = u64;

    #[inline]
    fn value_key(&self) -> u64 {
        // Adding 0.0 turns -0.0 into 0.0 and changes no other value, without a branch.
        let bits = (*self + 0.0).to_bits();
        if self.is_nan() {
            f64::NAN.to_bits()
        } else {
            bits
        }
    }
}

impl ValueKey for f32 {
    type Key = u32;

    #[inline]
    fn value_key(&self) -> u32 {
        // Adding 0.0 turns -0.0 into 0.0 and changes no other value, without a branch.
        let bits = (*self + 0.0).to_bits();
        if self.is_nan() {
            f32::NAN.to_bits()
        } else {
            bits
        }
    }
}

impl<T: ValueKey> ValueKey for Option<T> {
    type Key = Option<T::Key>;

    #[inline]
    fn value_key(&self) -> Self::Key {
        self.as_ref().map(ValueKey::value_key)
    }
}

impl<T: ValueKey, const N: usize> ValueKey for [T; N] {
    type Key = [T::Key; N];

    #[inline]
    fn value_key(&self) -> Self::Key {
        std::array::from_fn(|index| self[index].value_key())
    }
}

macro_rules! impl_value_key_identity {
    ($($type:ty),*) => {
        $(
            impl ValueKey for $type {
                type Key = $type;

                #[inline]
                fn value_key(&self) -> $type {
                    *self
                }
            }
        )*
    };
}

impl_value_key_identity!(
    (),
    bool,
    char,
    u8,
    i8,
    u16,
    i16,
    u32,
    i32,
    u64,
    i64,
    u128,
    i128,
    usize,
    isize
);

macro_rules! impl_value_key_tuple {
    ($($name:ident $index:tt),+) => {
        impl<$($name: ValueKey),+> ValueKey for ($($name,)+) {
            type Key = ($($name::Key,)+);

            #[inline]
            fn value_key(&self) -> Self::Key {
                ($(self.$index.value_key(),)+)
            }
        }
    };
}

impl_value_key_tuple!(A 0);
impl_value_key_tuple!(A 0, B 1);
impl_value_key_tuple!(A 0, B 1, C 2);
impl_value_key_tuple!(A 0, B 1, C 2, D 3);
impl_value_key_tuple!(A 0, B 1, C 2, D 3, E 4);
impl_value_key_tuple!(A 0, B 1, C 2, D 3, E 4, F 5);
impl_value_key_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
impl_value_key_tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);

/// Autoref-specialization probe used by `EqHash`: `(&KeyProbe::<T>::new()).probe_key(&value)`
/// resolves to [`ValueKey`] when `T` implements it, and to the reference itself, compared and
/// hashed through `T`'s own `Eq` and `Hash`, otherwise. Both probe traits must be in scope at
/// the call site.
#[doc(hidden)]
pub struct KeyProbe<T>(PhantomData<T>);

impl<T> KeyProbe<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Default for KeyProbe<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[doc(hidden)]
pub trait KeyProbeValue<T> {
    type Key: Eq + Hash;

    fn probe_key(&self, value: &T) -> Self::Key;
}

impl<T: ValueKey> KeyProbeValue<T> for KeyProbe<T> {
    type Key = T::Key;

    #[inline]
    fn probe_key(&self, value: &T) -> T::Key {
        value.value_key()
    }
}

#[doc(hidden)]
pub trait KeyProbeFallback<T> {
    fn probe_key<'a>(&self, value: &'a T) -> &'a T;
}

impl<T: Eq + Hash> KeyProbeFallback<T> for &KeyProbe<T> {
    #[inline]
    fn probe_key<'a>(&self, value: &'a T) -> &'a T {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_keys_fold_zeros_and_nans() {
        assert_eq!(0.0f64.value_key(), (-0.0f64).value_key());
        assert_eq!(f64::NAN.value_key(), (-f64::NAN).value_key());
        assert_eq!(
            f64::NAN.value_key(),
            f64::from_bits(0x7ff0_0000_0000_0001).value_key()
        );
        assert_ne!(1.0f64.value_key(), 2.0f64.value_key());
        assert_ne!(f64::INFINITY.value_key(), f64::NEG_INFINITY.value_key());
        assert_eq!(0.0f32.value_key(), (-0.0f32).value_key());
        assert_eq!(f32::NAN.value_key(), (-f32::NAN).value_key());
    }

    #[test]
    fn float_keys_are_the_bits_of_every_other_value() {
        assert_eq!((-0.0f64).value_key(), 0);
        assert_eq!((-0.0f32).value_key(), 0);
        let values = [
            1.0,
            -1.0,
            0.1,
            f64::MIN_POSITIVE,
            -f64::MIN_POSITIVE,
            f64::from_bits(1),
            -f64::from_bits(1),
            f64::MAX,
            f64::MIN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ];
        for value in values {
            assert_eq!(value.value_key(), value.to_bits());
        }
        // A coarse sweep of the f32 bit patterns, subnormals and infinities included.
        for bits in (0..=u32::MAX).step_by(9973) {
            let value = f32::from_bits(bits);
            if !value.is_nan() && value != 0.0 {
                assert_eq!(value.value_key(), bits);
            }
        }
    }

    #[test]
    fn compound_keys_follow_their_elements() {
        assert_eq!(Some(-0.0f64).value_key(), Some(0.0f64).value_key());
        assert_ne!(None::<f64>.value_key(), Some(0.0f64).value_key());
        assert_eq!(
            [0.0f64, f64::NAN].value_key(),
            [-0.0f64, f64::NAN].value_key()
        );
        assert_eq!((1u8, -0.0f32).value_key(), (1u8, 0.0f32).value_key());
    }

    #[test]
    fn probe_prefers_value_key_and_falls_back_to_hash() {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        struct Plain(u8);

        assert_eq!(KeyProbe::<f64>::new().probe_key(&-0.0), 0u64);
        assert_eq!((&KeyProbe::<Plain>::new()).probe_key(&Plain(3)), &Plain(3));
        assert_eq!(KeyProbe::<u16>::new().probe_key(&7), 7u16);
    }
}
