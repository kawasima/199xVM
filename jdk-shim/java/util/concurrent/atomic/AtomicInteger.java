/*
 * Copyright (c) 2003, 2024, Oracle and/or its affiliates. All rights reserved.
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

import java.util.function.IntBinaryOperator;
import java.util.function.IntUnaryOperator;

/**
 * An {@code int} value that may be updated atomically.
 *
 * <p>199xVM shim: uses volatile + synchronized instead of VarHandle/Unsafe.
 *
 * @since 1.5
 * @author Doug Lea
 */
public class AtomicInteger extends Number implements java.io.Serializable {

    private volatile int value;

    public AtomicInteger(int initialValue) {
        value = initialValue;
    }

    public AtomicInteger() {
    }

    public final int get() {
        return value;
    }

    public final void set(int newValue) {
        value = newValue;
    }

    public final void lazySet(int newValue) {
        value = newValue;
    }

    public final synchronized int getAndSet(int newValue) {
        int old = value;
        value = newValue;
        return old;
    }

    public final synchronized boolean compareAndSet(int expectedValue, int newValue) {
        if (value == expectedValue) {
            value = newValue;
            return true;
        }
        return false;
    }

    public final boolean weakCompareAndSet(int expectedValue, int newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final boolean weakCompareAndSetPlain(int expectedValue, int newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final synchronized int getAndIncrement() {
        return value++;
    }

    public final synchronized int getAndDecrement() {
        return value--;
    }

    public final synchronized int getAndAdd(int delta) {
        int old = value;
        value += delta;
        return old;
    }

    public final synchronized int incrementAndGet() {
        return ++value;
    }

    public final synchronized int decrementAndGet() {
        return --value;
    }

    public final synchronized int addAndGet(int delta) {
        value += delta;
        return value;
    }

    public final synchronized int getAndUpdate(IntUnaryOperator updateFunction) {
        int prev = value;
        value = updateFunction.applyAsInt(prev);
        return prev;
    }

    public final synchronized int updateAndGet(IntUnaryOperator updateFunction) {
        value = updateFunction.applyAsInt(value);
        return value;
    }

    public final synchronized int getAndAccumulate(int x, IntBinaryOperator accumulatorFunction) {
        int prev = value;
        value = accumulatorFunction.applyAsInt(prev, x);
        return prev;
    }

    public final synchronized int accumulateAndGet(int x, IntBinaryOperator accumulatorFunction) {
        value = accumulatorFunction.applyAsInt(value, x);
        return value;
    }

    public String toString() {
        return Integer.toString(get());
    }

    public int intValue() {
        return get();
    }

    public long longValue() {
        return (long) get();
    }

    public float floatValue() {
        return (float) get();
    }

    public double doubleValue() {
        return (double) get();
    }

    public final int getPlain() {
        return value;
    }

    public final void setPlain(int newValue) {
        value = newValue;
    }

    public final int getOpaque() {
        return value;
    }

    public final void setOpaque(int newValue) {
        value = newValue;
    }

    public final int getAcquire() {
        return value;
    }

    public final void setRelease(int newValue) {
        value = newValue;
    }

    public final synchronized int compareAndExchange(int expectedValue, int newValue) {
        int witness = value;
        if (witness == expectedValue) {
            value = newValue;
        }
        return witness;
    }

    public final int compareAndExchangeAcquire(int expectedValue, int newValue) {
        return compareAndExchange(expectedValue, newValue);
    }

    public final int compareAndExchangeRelease(int expectedValue, int newValue) {
        return compareAndExchange(expectedValue, newValue);
    }

    public final boolean weakCompareAndSetVolatile(int expectedValue, int newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final boolean weakCompareAndSetAcquire(int expectedValue, int newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final boolean weakCompareAndSetRelease(int expectedValue, int newValue) {
        return compareAndSet(expectedValue, newValue);
    }
}
