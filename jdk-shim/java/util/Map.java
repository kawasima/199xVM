package java.util;

public interface Map<K, V> {
    int size();
    boolean isEmpty();
    boolean containsKey(Object key);
    V get(Object key);
    V put(K key, V value);
    V remove(Object key);
    Set<K> keySet();
    Collection<V> values();
    Set<Entry<K, V>> entrySet();

    interface Entry<K, V> {
        K getKey();
        V getValue();
    }

    static <K, V> Map<K, V> of() {
        return new HashMap<>();
    }

    static <K, V> Map<K, V> of(K k1, V v1) {
        HashMap<K, V> m = new HashMap<>();
        m.put(k1, v1);
        return m;
    }

    static <K, V> Map<K, V> of(K k1, V v1, K k2, V v2) {
        HashMap<K, V> m = new HashMap<>();
        m.put(k1, v1);
        m.put(k2, v2);
        return m;
    }

    static <K, V> Map<K, V> of(K k1, V v1, K k2, V v2, K k3, V v3) {
        HashMap<K, V> m = new HashMap<>();
        m.put(k1, v1);
        m.put(k2, v2);
        m.put(k3, v3);
        return m;
    }
}
