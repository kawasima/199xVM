package java.util;

/**
 * Minimal HashMap using linear probing.
 * Not optimized — sufficient for small maps in decoder invocations.
 */
public class HashMap<K, V> implements Map<K, V> {
    private static final int DEFAULT_CAPACITY = 16;

    private Object[] keys;
    private Object[] values;
    private int size;

    public HashMap() {
        keys = new Object[DEFAULT_CAPACITY];
        values = new Object[DEFAULT_CAPACITY];
    }

    public HashMap(int initialCapacity) {
        keys = new Object[initialCapacity];
        values = new Object[initialCapacity];
    }

    private int indexOf(Object key) {
        for (int i = 0; i < size; i++) {
            if (Objects.equals(keys[i], key)) return i;
        }
        return -1;
    }

    private void grow() {
        int newCap = keys.length * 2;
        Object[] newKeys = new Object[newCap];
        Object[] newVals = new Object[newCap];
        for (int i = 0; i < size; i++) {
            newKeys[i] = keys[i];
            newVals[i] = values[i];
        }
        keys = newKeys;
        values = newVals;
    }

    @Override
    public int size() { return size; }

    @Override
    public boolean isEmpty() { return size == 0; }

    @Override
    public boolean containsKey(Object key) {
        return indexOf(key) >= 0;
    }

    @Override
    @SuppressWarnings("unchecked")
    public V get(Object key) {
        int i = indexOf(key);
        return i >= 0 ? (V) values[i] : null;
    }

    @Override
    @SuppressWarnings("unchecked")
    public V put(K key, V value) {
        int i = indexOf(key);
        if (i >= 0) {
            V old = (V) values[i];
            values[i] = value;
            return old;
        }
        if (size == keys.length) grow();
        keys[size] = key;
        values[size] = value;
        size++;
        return null;
    }

    @Override
    @SuppressWarnings("unchecked")
    public V remove(Object key) {
        int i = indexOf(key);
        if (i < 0) return null;
        V old = (V) values[i];
        // Shift last element into removed slot
        size--;
        keys[i] = keys[size];
        values[i] = values[size];
        keys[size] = null;
        values[size] = null;
        return old;
    }

    @Override
    public Set<K> keySet() {
        // Minimal: return null, implement when needed
        return null;
    }

    @Override
    public Collection<V> values() {
        return null;
    }

    @Override
    public Set<Entry<K, V>> entrySet() {
        return null;
    }

    @Override
    public String toString() {
        StringBuilder sb = new StringBuilder("{");
        for (int i = 0; i < size; i++) {
            if (i > 0) sb.append(", ");
            sb.append(keys[i]).append("=").append(values[i]);
        }
        sb.append("}");
        return sb.toString();
    }
}
