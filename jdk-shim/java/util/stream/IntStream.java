/*
 * Copyright (c) 2012, 2024, Oracle and/or its affiliates. All rights reserved.
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

import java.util.function.Consumer;
import java.util.function.Function;
import java.util.function.IntConsumer;

/**
 * Minimal IntStream shim — provides range(), parallel(), forEach(), mapToObj().
 */
public interface IntStream {

    void forEach(IntConsumer action);

    IntStream parallel();

    <U> Stream<U> mapToObj(java.util.function.IntFunction<? extends U> mapper);

    static IntStream range(int startInclusive, int endExclusive) {
        return new IntStream() {
            @Override
            public void forEach(IntConsumer action) {
                for (int i = startInclusive; i < endExclusive; i++) {
                    action.accept(i);
                }
            }

            @Override
            public IntStream parallel() {
                // No real parallelism — return self
                return this;
            }

            @Override
            public <U> Stream<U> mapToObj(java.util.function.IntFunction<? extends U> mapper) {
                java.util.List<U> result = new java.util.ArrayList<>();
                for (int i = startInclusive; i < endExclusive; i++) {
                    result.add(mapper.apply(i));
                }
                return new StreamImpl<>(result);
            }
        };
    }
}
