package java.util;

import java.util.function.BiConsumer;
import java.util.function.BiFunction;
import java.util.function.Function;

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

    default boolean containsValue(Object value) {
        for (Entry<K, V> e : entrySet()) {
            if (Objects.equals(e.getValue(), value)) return true;
        }
        return false;
    }

    default V getOrDefault(Object key, V defaultValue) {
        V v = get(key);
        return (v != null || containsKey(key)) ? v : defaultValue;
    }

    default V putIfAbsent(K key, V value) {
        V v = get(key);
        if (v == null) v = put(key, value);
        return v;
    }

    default boolean remove(Object key, Object value) {
        V cur = get(key);
        if (!Objects.equals(cur, value) || (cur == null && !containsKey(key))) return false;
        remove(key);
        return true;
    }

    default boolean replace(K key, V oldValue, V newValue) {
        V cur = get(key);
        if (!Objects.equals(cur, oldValue) || (cur == null && !containsKey(key))) return false;
        put(key, newValue);
        return true;
    }

    default V replace(K key, V value) {
        V cur = get(key);
        if (cur != null || containsKey(key)) {
            return put(key, value);
        }
        return null;
    }

    default V computeIfAbsent(K key, Function<? super K, ? extends V> mappingFunction) {
        V v = get(key);
        if (v == null) {
            V nv = mappingFunction.apply(key);
            if (nv != null) {
                put(key, nv);
                return nv;
            }
        }
        return v;
    }

    default V computeIfPresent(K key, BiFunction<? super K, ? super V, ? extends V> remappingFunction) {
        V oldValue = get(key);
        if (oldValue != null) {
            V newValue = remappingFunction.apply(key, oldValue);
            if (newValue != null) {
                put(key, newValue);
                return newValue;
            }
            remove(key);
        }
        return null;
    }

    default V compute(K key, BiFunction<? super K, ? super V, ? extends V> remappingFunction) {
        V oldValue = get(key);
        V newValue = remappingFunction.apply(key, oldValue);
        if (newValue == null) {
            if (oldValue != null || containsKey(key)) remove(key);
            return null;
        }
        put(key, newValue);
        return newValue;
    }

    default V merge(K key, V value, BiFunction<? super V, ? super V, ? extends V> remappingFunction) {
        V oldValue = get(key);
        V newValue = (oldValue == null) ? value : remappingFunction.apply(oldValue, value);
        if (newValue == null) {
            remove(key);
            return null;
        }
        put(key, newValue);
        return newValue;
    }

    default void forEach(BiConsumer<? super K, ? super V> action) {
        for (Entry<K, V> e : entrySet()) {
            action.accept(e.getKey(), e.getValue());
        }
    }

    default void replaceAll(BiFunction<? super K, ? super V, ? extends V> function) {
        for (Entry<K, V> e : entrySet()) {
            e.setValue(function.apply(e.getKey(), e.getValue()));
        }
    }

    interface Entry<K, V> {
        K getKey();
        V getValue();
        V setValue(V value);
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
