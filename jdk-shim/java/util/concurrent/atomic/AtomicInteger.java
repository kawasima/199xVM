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

package java.util.concurrent.atomic;

import java.io.Serializable;
import java.util.function.IntBinaryOperator;
import java.util.function.IntUnaryOperator;

public class AtomicInteger extends Number implements Serializable {
    private static final long serialVersionUID = 6214790243416807050L;

    private volatile int value;

    public AtomicInteger(int initialValue) {
        this.value = initialValue;
    }

    public AtomicInteger() {}

    public final int get() {
        return value;
    }

    public final void set(int newValue) {
        value = newValue;
    }

    public final void lazySet(int newValue) {
        value = newValue;
    }

    public final int getAndSet(int newValue) {
        int prev = value;
        value = newValue;
        return prev;
    }

    public final boolean compareAndSet(int expectedValue, int newValue) {
        if (value == expectedValue) {
            value = newValue;
            return true;
        }
        return false;
    }

    public final int getAndIncrement() {
        return getAndAdd(1);
    }

    public final int incrementAndGet() {
        return addAndGet(1);
    }

    public final int getAndDecrement() {
        return getAndAdd(-1);
    }

    public final int decrementAndGet() {
        return addAndGet(-1);
    }

    public final int getAndAdd(int delta) {
        int prev = value;
        value = prev + delta;
        return prev;
    }

    public final int addAndGet(int delta) {
        value = value + delta;
        return value;
    }

    public final int getAndUpdate(IntUnaryOperator updateFunction) {
        int prev = value;
        int next = updateFunction.applyAsInt(prev);
        value = next;
        return prev;
    }

    public final int updateAndGet(IntUnaryOperator updateFunction) {
        int prev = value;
        int next = updateFunction.applyAsInt(prev);
        value = next;
        return next;
    }

    public final int getAndAccumulate(int x, IntBinaryOperator accumulatorFunction) {
        int prev = value;
        int next = accumulatorFunction.applyAsInt(prev, x);
        value = next;
        return prev;
    }

    public final int accumulateAndGet(int x, IntBinaryOperator accumulatorFunction) {
        int prev = value;
        int next = accumulatorFunction.applyAsInt(prev, x);
        value = next;
        return next;
    }

    public int intValue() {
        return value;
    }

    public long longValue() {
        return value;
    }

    public float floatValue() {
        return value;
    }

    public double doubleValue() {
        return value;
    }

    public String toString() {
        return Integer.toString(value);
    }
}
