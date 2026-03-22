//! VM-wide map/set selection.
//!
//! `fast-hash` favors throughput for trusted local workloads by using Fx hashers.
//! Disable that feature to fall back to the standard random-state hashers when
//! hash-flooding resistance matters more than raw interpreter speed.

#[cfg(feature = "fast-hash")]
pub(crate) type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;

#[cfg(not(feature = "fast-hash"))]
pub(crate) type HashMap<K, V> = std::collections::HashMap<K, V>;

#[cfg(feature = "fast-hash")]
pub(crate) type HashSet<T> = rustc_hash::FxHashSet<T>;

#[cfg(not(feature = "fast-hash"))]
pub(crate) type HashSet<T> = std::collections::HashSet<T>;

pub(crate) fn hash_map_with_capacity<K, V>(capacity: usize) -> HashMap<K, V> {
    HashMap::with_capacity_and_hasher(capacity, Default::default())
}
