package java.util.concurrent;

import java.io.Serializable;
import java.util.AbstractCollection;
import java.util.ArrayDeque;
import java.util.Collection;
import java.util.Iterator;
import java.util.NoSuchElementException;
import java.util.Queue;

public class ConcurrentLinkedQueue<E> extends AbstractCollection<E> implements Queue<E>, Serializable {
    private static final long serialVersionUID = 196745693267521676L;

    private final ArrayDeque<E> data;

    public ConcurrentLinkedQueue() {
        this.data = new ArrayDeque<>();
    }

    public ConcurrentLinkedQueue(Collection<? extends E> c) {
        this.data = new ArrayDeque<>(c);
    }

    public boolean add(E e) {
        return data.add(e);
    }

    public boolean offer(E e) {
        return data.offer(e);
    }

    public E remove() {
        return data.remove();
    }

    public E poll() {
        return data.poll();
    }

    public E element() {
        return data.element();
    }

    public E peek() {
        return data.peek();
    }

    public boolean remove(Object o) {
        return data.remove(o);
    }

    public boolean contains(Object o) {
        return data.contains(o);
    }

    public int size() {
        return data.size();
    }

    public boolean isEmpty() {
        return data.isEmpty();
    }

    public Iterator<E> iterator() {
        return data.iterator();
    }

    public void clear() {
        data.clear();
    }
}
