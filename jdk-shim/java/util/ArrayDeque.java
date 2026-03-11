package java.util;

import java.io.Serializable;

public class ArrayDeque<E> extends AbstractCollection<E> implements Cloneable, Serializable {
    private static final long serialVersionUID = 2340985798034038923L;

    private final ArrayList<E> data;

    public ArrayDeque() {
        this.data = new ArrayList<>();
    }

    public ArrayDeque(int numElements) {
        this.data = new ArrayList<>(numElements < 0 ? 0 : numElements);
    }

    public ArrayDeque(Collection<? extends E> c) {
        this.data = new ArrayList<>(c.size());
        addAll(c);
    }

    public void addFirst(E e) {
        if (e == null) throw new NullPointerException();
        data.add(0, e);
    }

    public void addLast(E e) {
        if (e == null) throw new NullPointerException();
        data.add(e);
    }

    public boolean offerFirst(E e) {
        addFirst(e);
        return true;
    }

    public boolean offerLast(E e) {
        addLast(e);
        return true;
    }

    public E removeFirst() {
        if (data.isEmpty()) throw new NoSuchElementException();
        return data.remove(0);
    }

    public E removeLast() {
        if (data.isEmpty()) throw new NoSuchElementException();
        return data.remove(data.size() - 1);
    }

    public E pollFirst() {
        if (data.isEmpty()) return null;
        return data.remove(0);
    }

    public E pollLast() {
        if (data.isEmpty()) return null;
        return data.remove(data.size() - 1);
    }

    public E getFirst() {
        if (data.isEmpty()) throw new NoSuchElementException();
        return data.get(0);
    }

    public E getLast() {
        if (data.isEmpty()) throw new NoSuchElementException();
        return data.get(data.size() - 1);
    }

    public E peekFirst() {
        return data.isEmpty() ? null : data.get(0);
    }

    public E peekLast() {
        return data.isEmpty() ? null : data.get(data.size() - 1);
    }

    public boolean removeFirstOccurrence(Object o) {
        return data.remove(o);
    }

    public boolean removeLastOccurrence(Object o) {
        for (int i = data.size() - 1; i >= 0; i--) {
            E e = data.get(i);
            if (o == null ? e == null : o.equals(e)) {
                data.remove(i);
                return true;
            }
        }
        return false;
    }

    public boolean add(E e) {
        addLast(e);
        return true;
    }

    public boolean offer(E e) {
        return offerLast(e);
    }

    public E remove() {
        return removeFirst();
    }

    public E poll() {
        return pollFirst();
    }

    public E element() {
        return getFirst();
    }

    public E peek() {
        return peekFirst();
    }

    public void push(E e) {
        addFirst(e);
    }

    public E pop() {
        return removeFirst();
    }

    public int size() {
        return data.size();
    }

    public boolean isEmpty() {
        return data.isEmpty();
    }

    public boolean contains(Object o) {
        return data.contains(o);
    }

    public Iterator<E> iterator() {
        return data.iterator();
    }

    public Iterator<E> descendingIterator() {
        return new Iterator<E>() {
            private int index = data.size() - 1;

            public boolean hasNext() {
                return index >= 0;
            }

            public E next() {
                if (!hasNext()) throw new NoSuchElementException();
                return data.get(index--);
            }

            public void remove() {
                throw new UnsupportedOperationException();
            }
        };
    }

    public Object[] toArray() {
        return data.toArray();
    }

    public <T> T[] toArray(T[] a) {
        return data.toArray(a);
    }

    public boolean remove(Object o) {
        return removeFirstOccurrence(o);
    }

    public void clear() {
        data.clear();
    }

    @SuppressWarnings("unchecked")
    public ArrayDeque<E> clone() {
        return new ArrayDeque<>(this.data);
    }
}
