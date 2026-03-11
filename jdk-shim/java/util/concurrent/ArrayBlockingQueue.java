package java.util.concurrent;

import java.io.Serializable;
import java.util.AbstractQueue;
import java.util.ArrayDeque;
import java.util.Collection;
import java.util.Iterator;

public class ArrayBlockingQueue<E> extends AbstractQueue<E>
    implements BlockingQueue<E>, Serializable {

    private static final long serialVersionUID = -817911632652898426L;

    private final ArrayDeque<E> data;
    private final int capacity;

    public ArrayBlockingQueue(int capacity) {
        this(capacity, false);
    }

    public ArrayBlockingQueue(int capacity, boolean fair) {
        if (capacity <= 0) throw new IllegalArgumentException();
        this.capacity = capacity;
        this.data = new ArrayDeque<>(capacity);
    }

    public ArrayBlockingQueue(int capacity, boolean fair, Collection<? extends E> c) {
        this(capacity, fair);
        addAll(c);
    }

    public boolean add(E e) {
        return super.add(e);
    }

    public boolean offer(E e) {
        if (e == null) throw new NullPointerException();
        if (data.size() >= capacity) return false;
        data.addLast(e);
        return true;
    }

    public void put(E e) throws InterruptedException {
        if (!offer(e)) throw new InterruptedException("Queue full");
    }

    public boolean offer(E e, long timeout, TimeUnit unit) throws InterruptedException {
        return offer(e);
    }

    public E poll() {
        return data.pollFirst();
    }

    public E take() throws InterruptedException {
        E v = poll();
        if (v != null) return v;
        throw new InterruptedException("Queue empty");
    }

    public E poll(long timeout, TimeUnit unit) throws InterruptedException {
        return poll();
    }

    public E peek() {
        return data.peekFirst();
    }

    public int size() {
        return data.size();
    }

    public int remainingCapacity() {
        return capacity - data.size();
    }

    public boolean remove(Object o) {
        return data.remove(o);
    }

    public boolean contains(Object o) {
        return data.contains(o);
    }

    public Object[] toArray() {
        return data.toArray();
    }

    public <T> T[] toArray(T[] a) {
        return data.toArray(a);
    }

    public String toString() {
        return data.toString();
    }

    public void clear() {
        data.clear();
    }

    public int drainTo(Collection<? super E> c) {
        return drainTo(c, Integer.MAX_VALUE);
    }

    public int drainTo(Collection<? super E> c, int maxElements) {
        if (c == null) throw new NullPointerException();
        if (c == this) throw new IllegalArgumentException();
        int n = 0;
        while (n < maxElements) {
            E e = data.pollFirst();
            if (e == null) break;
            c.add(e);
            n++;
        }
        return n;
    }

    public Iterator<E> iterator() {
        return data.iterator();
    }
}
