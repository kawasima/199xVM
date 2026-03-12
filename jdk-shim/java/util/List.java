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
    ListIterator<E> listIterator();
    ListIterator<E> listIterator(int index);
    List<E> subList(int fromIndex, int toIndex);

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
