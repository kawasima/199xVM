/*
 * Copyright (c) 1996, 2024, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */

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
