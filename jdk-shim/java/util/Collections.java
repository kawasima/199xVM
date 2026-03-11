package java.util;

public class Collections {
    private Collections() {}

    @SuppressWarnings("unchecked")
    public static <T> List<T> emptyList() {
        return (List<T>) EMPTY_LIST;
    }

    @SuppressWarnings("unchecked")
    public static <T> Set<T> emptySet() {
        return (Set<T>) EMPTY_SET;
    }

    @SuppressWarnings("unchecked")
    public static <K, V> Map<K, V> emptyMap() {
        return (Map<K, V>) EMPTY_MAP;
    }

    public static <T> List<T> unmodifiableList(List<? extends T> list) {
        return new UnmodifiableList<>(list);
    }

    public static <T> List<T> singletonList(T o) {
        ArrayList<T> list = new ArrayList<>();
        list.add(o);
        return unmodifiableList(list);
    }

    @SuppressWarnings("unchecked")
    public static <T> Iterator<T> emptyIterator() {
        return (Iterator<T>) EMPTY_LIST.iterator();
    }

    public static <K, V> Map<K, V> singletonMap(K key, V value) {
        HashMap<K, V> map = new HashMap<>(1);
        map.put(key, value);
        return map;
    }

    public static <T> Set<T> unmodifiableSet(Set<? extends T> set) {
        HashSet<T> copy = new HashSet<>();
        for (T e : set) {
            copy.add(e);
        }
        return new UnmodifiableSet<>(copy);
    }

    public static void reverse(List<?> list) {
        int size = list.size();
        for (int i = 0; i < size / 2; i++) {
            swap(list, i, size - 1 - i);
        }
    }

    @SuppressWarnings("unchecked")
    public static <T extends Comparable<? super T>> Comparator<T> reverseOrder() {
        return (a, b) -> b.compareTo(a);
    }

    public static <T> Comparator<T> reverseOrder(Comparator<T> cmp) {
        if (cmp == null) throw new NullPointerException();
        return (a, b) -> cmp.compare(b, a);
    }

    @SuppressWarnings("unchecked")
    private static void swap(List<?> list, int i, int j) {
        List rawList = list;
        Object tmp = rawList.get(i);
        rawList.set(i, rawList.get(j));
        rawList.set(j, tmp);
    }

    private static final List<?> EMPTY_LIST = new UnmodifiableList<>(new ArrayList<>());
    private static final Set<?> EMPTY_SET = new UnmodifiableSet<>(new HashSet<>());
    private static final Map<?, ?> EMPTY_MAP = new HashMap<>();

    private static class UnmodifiableList<E> implements List<E> {
        private final List<? extends E> list;

        UnmodifiableList(List<? extends E> list) {
            this.list = list;
        }

        @Override public int size() { return list.size(); }
        @Override public boolean isEmpty() { return list.isEmpty(); }
        @Override public boolean contains(Object o) { return list.contains(o); }
        @Override public E get(int index) { return list.get(index); }

        @Override public E set(int index, E element) {
            throw new UnsupportedOperationException();
        }
        @Override public boolean add(E e) {
            throw new UnsupportedOperationException();
        }
        @Override public void add(int index, E element) {
            throw new UnsupportedOperationException();
        }
        @Override public boolean addAll(Collection<? extends E> c) {
            throw new UnsupportedOperationException();
        }
        @Override public E remove(int index) {
            throw new UnsupportedOperationException();
        }
        @Override public boolean remove(Object o) {
            throw new UnsupportedOperationException();
        }
        @Override public boolean removeAll(Collection<?> c) {
            throw new UnsupportedOperationException();
        }
        @Override public boolean retainAll(Collection<?> c) {
            throw new UnsupportedOperationException();
        }
        @Override public boolean addAll(int index, Collection<? extends E> c) {
            throw new UnsupportedOperationException();
        }
        @Override public boolean containsAll(Collection<?> c) {
            for (Object e : c) {
                if (!contains(e)) return false;
            }
            return true;
        }
        @Override public Object[] toArray() {
            return list.toArray();
        }
        @Override @SuppressWarnings("unchecked")
        public <T> T[] toArray(T[] a) {
            return list.toArray(a);
        }
        @Override public int indexOf(Object o) {
            return list.indexOf(o);
        }
        @Override public int lastIndexOf(Object o) {
            return list.lastIndexOf(o);
        }
        @Override public void clear() {
            throw new UnsupportedOperationException();
        }
        @Override @SuppressWarnings("unchecked")
        public ListIterator<E> listIterator() {
            return (ListIterator<E>) list.listIterator();
        }
        @Override @SuppressWarnings("unchecked")
        public ListIterator<E> listIterator(int index) {
            return (ListIterator<E>) list.listIterator(index);
        }
        @Override
        public List<E> subList(int fromIndex, int toIndex) {
            return new UnmodifiableList<>(list.subList(fromIndex, toIndex));
        }

        @Override
        @SuppressWarnings("unchecked")
        public Iterator<E> iterator() {
            return (Iterator<E>) list.iterator();
        }

        @Override
        public String toString() { return list.toString(); }
    }

    private static class UnmodifiableSet<E> implements Set<E> {
        private final Set<? extends E> set;

        UnmodifiableSet(Set<? extends E> set) {
            this.set = set;
        }

        @Override public int size() { return set.size(); }
        @Override public boolean isEmpty() { return set.isEmpty(); }
        @Override public boolean contains(Object o) { return set.contains(o); }
        @Override public Iterator<E> iterator() { @SuppressWarnings("unchecked") Iterator<E> it = (Iterator<E>) set.iterator(); return it; }
        @Override public Object[] toArray() { return set.toArray(); }
        @Override public <T> T[] toArray(T[] a) { return set.toArray(a); }
        @Override public boolean add(E e) { throw new UnsupportedOperationException(); }
        @Override public boolean remove(Object o) { throw new UnsupportedOperationException(); }
        @Override public boolean containsAll(Collection<?> c) { return set.containsAll(c); }
        @Override public boolean addAll(Collection<? extends E> c) { throw new UnsupportedOperationException(); }
        @Override public boolean retainAll(Collection<?> c) { throw new UnsupportedOperationException(); }
        @Override public boolean removeAll(Collection<?> c) { throw new UnsupportedOperationException(); }
        @Override public void clear() { throw new UnsupportedOperationException(); }
    }
}
