package java.util;

public class HashSet<E> implements Set<E> {
    private static final Object PRESENT = new Object();
    private final HashMap<E, Object> map;

    public HashSet() {
        this.map = new HashMap<>();
    }

    public HashSet(Collection<? extends E> c) {
        this();
        addAll(c);
    }

    public HashSet(int initialCapacity) {
        this.map = new HashMap<>(initialCapacity);
    }

    public HashSet(int initialCapacity, float loadFactor) {
        this.map = new HashMap<>(initialCapacity);
    }

    public static <E> HashSet<E> newHashSet(int expectedSize) {
        return new HashSet<>(Math.max(expectedSize, 16));
    }

    @Override
    public Iterator<E> iterator() {
        return map.keySet().iterator();
    }

    @Override
    public int size() {
        return map.size();
    }

    @Override
    public boolean isEmpty() {
        return map.isEmpty();
    }

    @Override
    public boolean contains(Object o) {
        return map.containsKey(o);
    }

    @Override
    public boolean add(E e) {
        return map.put(e, PRESENT) == null;
    }

    @Override
    public boolean remove(Object o) {
        return map.remove(o) == PRESENT;
    }

    @Override
    public void clear() {
        map.clear();
    }

    @Override
    public boolean addAll(Collection<? extends E> c) {
        boolean modified = false;
        for (E e : c) {
            if (add(e)) modified = true;
        }
        return modified;
    }
}
