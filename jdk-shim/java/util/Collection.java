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

import java.util.function.Consumer;
import java.util.function.IntFunction;
import java.util.function.Predicate;
import java.util.stream.Stream;
import java.util.stream.StreamImpl;

public interface Collection<E> extends Iterable<E> {
    int size();
    boolean isEmpty();
    boolean contains(Object o);
    Iterator<E> iterator();
    boolean add(E e);
    boolean addAll(Collection<? extends E> c);
    void clear();

    default boolean remove(Object o) {
        Iterator<E> it = iterator();
        while (it.hasNext()) {
            if (Objects.equals(it.next(), o)) {
                it.remove();
                return true;
            }
        }
        return false;
    }

    default boolean containsAll(Collection<?> c) {
        for (Object e : c) {
            if (!contains(e)) return false;
        }
        return true;
    }

    default boolean removeAll(Collection<?> c) {
        boolean modified = false;
        Iterator<E> it = iterator();
        while (it.hasNext()) {
            if (c.contains(it.next())) {
                it.remove();
                modified = true;
            }
        }
        return modified;
    }

    default boolean retainAll(Collection<?> c) {
        boolean modified = false;
        Iterator<E> it = iterator();
        while (it.hasNext()) {
            if (!c.contains(it.next())) {
                it.remove();
                modified = true;
            }
        }
        return modified;
    }

    default Object[] toArray() {
        Object[] result = new Object[size()];
        Iterator<E> it = iterator();
        for (int i = 0; i < result.length && it.hasNext(); i++) {
            result[i] = it.next();
        }
        return result;
    }

    @SuppressWarnings("unchecked")
    default <T> T[] toArray(T[] a) {
        int s = size();
        Iterator<E> it = iterator();
        for (int i = 0; i < s && i < a.length && it.hasNext(); i++) {
            a[i] = (T) it.next();
        }
        if (a.length > s) a[s] = null;
        return a;
    }

    default <T> T[] toArray(IntFunction<T[]> generator) {
        return toArray(generator.apply(size()));
    }

    default void forEach(Consumer<? super E> action) {
        Iterator<E> it = iterator();
        while (it.hasNext()) action.accept(it.next());
    }

    default boolean removeIf(Predicate<? super E> filter) {
        boolean removed = false;
        Iterator<E> it = iterator();
        while (it.hasNext()) {
            if (filter.test(it.next())) {
                it.remove();
                removed = true;
            }
        }
        return removed;
    }

    default Spliterator<E> spliterator() {
        return null;
    }

    default Stream<E> stream() {
        ArrayList<E> copy = new ArrayList<>();
        Iterator<E> it = iterator();
        while (it.hasNext()) copy.add(it.next());
        return new StreamImpl<>(copy);
    }

    default Stream<E> parallelStream() {
        return stream();
    }
}
