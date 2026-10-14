//! The [`define_atomic_id!`](crate::define_atomic_id) macro.

/// Declares an id newtype over `u64`.
///
/// `define_atomic_id!(FooId)` makes a `Copy` struct `FooId` whose `new()` hands out ids from a
/// per-type atomic counter starting at 0, so every call returns a different value for the life of
/// the process. `FooId::INVALID` is `u64::MAX` and is never handed out by `new()`.
/// `from_u64` and `from_u128` (hashes a UUID down to 64 bits) build ids from fixed values, and
/// are `const`; those can collide with counter ids.
///
/// Attributes (including `///` docs) written before the name are applied to the struct.
///
/// # Examples
///
/// ```
/// weaver_util::define_atomic_id!(
///     /// Identifies a widget.
///     WidgetId
/// );
///
/// let a = WidgetId::new();
/// assert_ne!(a, WidgetId::new());
/// assert!(a.is_valid());
/// ```
#[macro_export]
macro_rules! define_atomic_id {
    ($(#[$meta:meta])* $id:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $id(u64);

        impl $id {
            /// Sentinel id that [`Self::new`] never returns.
            pub const INVALID: Self = Self(u64::MAX);

            /// Returns a fresh id from this type's process-wide counter.
            #[allow(clippy::new_without_default)]
            pub fn new() -> Self {
                static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                Self(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
            }

            /// Returns `true` unless this is [`Self::INVALID`].
            pub fn is_valid(&self) -> bool {
                *self != Self::INVALID
            }

            /// Wraps a fixed value. Can collide with ids from [`Self::new`].
            pub const fn from_u64(id: u64) -> Self {
                Self(id)
            }

            /// Hashes a 128-bit value (such as a UUID) down to an id. Can collide with ids from
            /// [`Self::new`].
            pub const fn from_u128(uuid: u128) -> Self {
                Self($crate::maps::fast_hash_u128_const(uuid))
            }

            /// Returns the raw value.
            pub const fn as_u64(&self) -> u64 {
                self.0
            }

            /// Returns the raw value as a `usize`.
            pub const fn as_usize(&self) -> usize {
                self.0 as usize
            }
        }

        impl Into<u64> for $id {
            fn into(self) -> u64 {
                self.0
            }
        }

        impl Into<usize> for $id {
            fn into(self) -> usize {
                self.0 as usize
            }
        }
    };
}
