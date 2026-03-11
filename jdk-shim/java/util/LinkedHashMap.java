package java.util;

public class LinkedHashMap<K, V> extends HashMap<K, V> {
    public LinkedHashMap() {
        super();
    }

    public LinkedHashMap(int initialCapacity) {
        super(initialCapacity);
    }

    public LinkedHashMap(int initialCapacity, float loadFactor) {
        super(initialCapacity);
    }

    public LinkedHashMap(Map<? extends K, ? extends V> m) {
        super(m);
    }
}
