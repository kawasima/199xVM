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
import java.util.function.LongBinaryOperator;
import java.util.function.LongUnaryOperator;

public class AtomicLong extends Number implements Serializable {
    private static final long serialVersionUID = 1927816293512124184L;

    private volatile long value;

    public AtomicLong(long initialValue) {
        this.value = initialValue;
    }

    public AtomicLong() {}

    public final long get() {
        return value;
    }

    public final void set(long newValue) {
        value = newValue;
    }

    public final void lazySet(long newValue) {
        value = newValue;
    }

    public final long getAndSet(long newValue) {
        long prev = value;
        value = newValue;
        return prev;
    }

    public final boolean compareAndSet(long expectedValue, long newValue) {
        if (value == expectedValue) {
            value = newValue;
            return true;
        }
        return false;
    }

    @Deprecated(since="9")
    public final boolean weakCompareAndSet(long expectedValue, long newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final boolean weakCompareAndSetPlain(long expectedValue, long newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final long getAndIncrement() {
        return getAndAdd(1L);
    }

    public final long getAndDecrement() {
        return getAndAdd(-1L);
    }

    public final long getAndAdd(long delta) {
        long prev = value;
        value = prev + delta;
        return prev;
    }

    public final long incrementAndGet() {
        return addAndGet(1L);
    }

    public final long decrementAndGet() {
        return addAndGet(-1L);
    }

    public final long addAndGet(long delta) {
        value = value + delta;
        return value;
    }

    public final long getAndUpdate(LongUnaryOperator updateFunction) {
        long prev = value;
        long next = updateFunction.applyAsLong(prev);
        value = next;
        return prev;
    }

    public final long updateAndGet(LongUnaryOperator updateFunction) {
        long prev = value;
        long next = updateFunction.applyAsLong(prev);
        value = next;
        return next;
    }

    public final long getAndAccumulate(long x, LongBinaryOperator accumulatorFunction) {
        long prev = value;
        long next = accumulatorFunction.applyAsLong(prev, x);
        value = next;
        return prev;
    }

    public final long accumulateAndGet(long x, LongBinaryOperator accumulatorFunction) {
        long prev = value;
        long next = accumulatorFunction.applyAsLong(prev, x);
        value = next;
        return next;
    }

    public int intValue() {
        return (int) value;
    }

    public long longValue() {
        return value;
    }

    public float floatValue() {
        return (float) value;
    }

    public double doubleValue() {
        return (double) value;
    }

    public String toString() {
        return Long.toString(value);
    }
}
