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
import java.util.AbstractCollection;
import java.util.ArrayDeque;
import java.util.Collection;
import java.util.Iterator;
import java.util.NoSuchElementException;

public class ConcurrentLinkedDeque<E> extends AbstractCollection<E> implements Serializable {
    private static final long serialVersionUID = 876323262645176354L;

    private final ArrayDeque<E> data;

    public ConcurrentLinkedDeque() {
        this.data = new ArrayDeque<>();
    }

    public ConcurrentLinkedDeque(Collection<? extends E> c) {
        this.data = new ArrayDeque<>(c);
    }

    public void addFirst(E e) {
        data.addFirst(e);
    }

    public void addLast(E e) {
        data.addLast(e);
    }

    public boolean offerFirst(E e) {
        return data.offerFirst(e);
    }

    public boolean offerLast(E e) {
        return data.offerLast(e);
    }

    public E removeFirst() {
        return data.removeFirst();
    }

    public E removeLast() {
        return data.removeLast();
    }

    public E pollFirst() {
        return data.pollFirst();
    }

    public E pollLast() {
        return data.pollLast();
    }

    public E getFirst() {
        return data.getFirst();
    }

    public E getLast() {
        return data.getLast();
    }

    public E peekFirst() {
        return data.peekFirst();
    }

    public E peekLast() {
        return data.peekLast();
    }

    public boolean removeFirstOccurrence(Object o) {
        return data.removeFirstOccurrence(o);
    }

    public boolean removeLastOccurrence(Object o) {
        return data.removeLastOccurrence(o);
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

    public void push(E e) {
        data.push(e);
    }

    public E pop() {
        return data.pop();
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

    public Iterator<E> descendingIterator() {
        return data.descendingIterator();
    }

    public void clear() {
        data.clear();
    }
}
