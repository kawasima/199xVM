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

package java.util.concurrent;

import java.io.Serializable;
import java.util.AbstractQueue;
import java.util.ArrayDeque;
import java.util.Collection;
import java.util.Iterator;

public class LinkedBlockingQueue<E> extends AbstractQueue<E>
    implements BlockingQueue<E>, Serializable {

    private static final long serialVersionUID = -6903933977591709194L;

    private final ArrayDeque<E> data;
    private final int capacity;

    public LinkedBlockingQueue() {
        this(Integer.MAX_VALUE);
    }

    public LinkedBlockingQueue(int capacity) {
        if (capacity <= 0) throw new IllegalArgumentException();
        this.capacity = capacity;
        this.data = new ArrayDeque<>();
    }

    public LinkedBlockingQueue(Collection<? extends E> c) {
        this(Integer.MAX_VALUE);
        addAll(c);
    }

    public int size() {
        return data.size();
    }

    public int remainingCapacity() {
        return capacity - data.size();
    }

    public void put(E e) throws InterruptedException {
        if (!offer(e)) throw new InterruptedException("Queue full");
    }

    public boolean offer(E e, long timeout, TimeUnit unit) throws InterruptedException {
        return offer(e);
    }

    public boolean offer(E e) {
        if (e == null) throw new NullPointerException();
        if (data.size() >= capacity) return false;
        data.addLast(e);
        return true;
    }

    public E take() throws InterruptedException {
        E v = poll();
        if (v != null) return v;
        throw new InterruptedException("Queue empty");
    }

    public E poll(long timeout, TimeUnit unit) throws InterruptedException {
        return poll();
    }

    public E poll() {
        return data.pollFirst();
    }

    public E peek() {
        return data.peekFirst();
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
