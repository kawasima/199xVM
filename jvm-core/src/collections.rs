//! Thin std collection aliases for VM internals.

pub(crate) type HashMap<K, V> = std::collections::HashMap<K, V>;
pub(crate) type HashSet<T> = std::collections::HashSet<T>;

pub(crate) fn hash_map_with_capacity<K, V>(capacity: usize) -> HashMap<K, V> {
    HashMap::with_capacity(capacity)
}
