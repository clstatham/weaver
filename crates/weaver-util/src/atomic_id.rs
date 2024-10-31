#[macro_export]
macro_rules! define_atomic_id {
    ($id:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $id(u64);

        impl $id {
            pub const INVALID: Self = Self(u64::MAX);

            #[allow(clippy::new_without_default)]
            pub fn new() -> Self {
                static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                Self(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
            }

            pub fn is_valid(&self) -> bool {
                *self != Self::INVALID
            }

            pub const fn from_u64(id: u64) -> Self {
                Self(id)
            }

            pub const fn from_u128(uuid: u128) -> Self {
                Self($crate::maps::fast_hash_u128_const(uuid))
            }

            pub const fn as_u64(&self) -> u64 {
                self.0
            }

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
