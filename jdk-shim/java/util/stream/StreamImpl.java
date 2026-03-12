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

package java.util.stream;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;
import java.util.function.Predicate;

public class StreamImpl<T> implements Stream<T> {
    private final List<T> elements;

    public StreamImpl(List<T> elements) {
        this.elements = elements;
    }

    @Override
    @SuppressWarnings("unchecked")
    public <R> Stream<R> map(Function<? super T, ? extends R> mapper) {
        List<R> result = new ArrayList<>();
        for (T e : elements) {
            result.add(mapper.apply(e));
        }
        return new StreamImpl<>(result);
    }

    @Override
    public Stream<T> filter(Predicate<? super T> predicate) {
        List<T> result = new ArrayList<>();
        for (T e : elements) {
            if (predicate.test(e)) result.add(e);
        }
        return new StreamImpl<>(result);
    }

    @Override
    @SuppressWarnings("unchecked")
    public <R> R collect(Collector<? super T, ?, R> collector) {
        Collector<T, Object, R> c = (Collector<T, Object, R>) collector;
        Object container = c.supplier();
        for (T e : elements) {
            c.accumulator(container, e);
        }
        return c.finisher(container);
    }
}
