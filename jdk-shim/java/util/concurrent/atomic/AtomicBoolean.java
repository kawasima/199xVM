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

/**
 * A {@code boolean} value that may be updated atomically.
 *
 * <p>Shim implementation for 199xVM -- the VM is single-threaded today,
 * so atomic operations use simple volatile field access.
 *
 * @since 1.5
 */
public class AtomicBoolean implements java.io.Serializable {
    private static final long serialVersionUID = 4654671469794556979L;

    private volatile boolean value;

    public AtomicBoolean(boolean initialValue) {
        value = initialValue;
    }

    public AtomicBoolean() {
    }

    public final boolean get() {
        return value;
    }

    public final boolean compareAndSet(boolean expectedValue, boolean newValue) {
        if (value == expectedValue) {
            value = newValue;
            return true;
        }
        return false;
    }

    public boolean weakCompareAndSetPlain(boolean expectedValue, boolean newValue) {
        return compareAndSet(expectedValue, newValue);
    }

    public final void set(boolean newValue) {
        value = newValue;
    }

    public final void lazySet(boolean newValue) {
        value = newValue;
    }

    public final boolean getAndSet(boolean newValue) {
        boolean prev = value;
        value = newValue;
        return prev;
    }

    public boolean getPlain() {
        return value;
    }

    public void setPlain(boolean newValue) {
        value = newValue;
    }

    public boolean getOpaque() {
        return value;
    }

    public void setOpaque(boolean newValue) {
        value = newValue;
    }

    public boolean getAcquire() {
        return value;
    }

    public void setRelease(boolean newValue) {
        value = newValue;
    }

    public boolean compareAndExchange(boolean expectedValue, boolean newValue) {
        boolean witness = value;
        if (witness == expectedValue) {
            value = newValue;
        }
        return witness;
    }

    @Override
    public String toString() {
        return get() ? "true" : "false";
    }
}
