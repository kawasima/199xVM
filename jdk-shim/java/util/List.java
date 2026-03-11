package java.util;

import java.util.stream.Stream;
import java.util.stream.StreamImpl;
import java.util.Iterator;

public interface List<E> extends Collection<E> {
    E get(int index);
    E set(int index, E element);
    void add(int index, E element);
    E remove(int index);
    boolean addAll(int index, Collection<? extends E> c);
    int indexOf(Object o);
    int lastIndexOf(Object o);

    default Stream<E> stream() {
        ArrayList<E> copy = new ArrayList<>();
        Iterator<E> it = iterator();
        while (it.hasNext()) {
            copy.add(it.next());
        }
        return new StreamImpl<>(copy);
    }

    @SafeVarargs
    static <E> List<E> of(E... elements) {
        ArrayList<E> list = new ArrayList<>();
        for (E e : elements) {
            list.add(e);
        }
        return Collections.unmodifiableList(list);
    }

    static <E> List<E> copyOf(Collection<? extends E> coll) {
        ArrayList<E> list = new ArrayList<>();
        for (E e : coll) {
            list.add(e);
        }
        return Collections.unmodifiableList(list);
    }
}
