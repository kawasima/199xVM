/*
 * Copyright (c) 1997, 2025, Oracle and/or its affiliates. All rights reserved.
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

package java.lang.ref;

/**
 * Minimal shim implementation of {@code java.lang.ref.ReferenceQueue}.
 *
 * <p>Queue bookkeeping is intentionally kept close to the JDK state machine so
 * future GC integration can enqueue cleared references through the same path.
 */
public class ReferenceQueue<T> {
    private static class Null extends ReferenceQueue<Object> {
        @Override
        boolean enqueue(Reference<?> r) {
            return false;
        }
    }

    static final ReferenceQueue<Object> NULL_QUEUE = new Null();
    static final ReferenceQueue<Object> ENQUEUED = new Null();

    private volatile Reference<? extends T> head;
    private long queueLength = 0;

    private static class Lock {}
    private final Lock lock = new Lock();

    public ReferenceQueue() {}

    private boolean enqueue0(Reference<? extends T> r) {
        ReferenceQueue<?> queue = r.queue;
        if (queue == NULL_QUEUE || queue == ENQUEUED) {
            return false;
        }

        r.next = (head == null) ? r : head;
        head = r;
        queueLength++;
        r.queue = ENQUEUED;
        lock.notifyAll();
        return true;
    }

    private Reference<? extends T> poll0() {
        Reference<? extends T> r = head;
        if (r == null) {
            return null;
        }

        r.queue = NULL_QUEUE;
        @SuppressWarnings("unchecked")
        Reference<? extends T> rn = r.next;
        head = (rn == r) ? null : rn;
        r.next = r;
        queueLength--;
        return r;
    }

    boolean enqueue(Reference<? extends T> r) {
        synchronized (lock) {
            return enqueue0(r);
        }
    }

    public Reference<? extends T> poll() {
        if (head == null) {
            return null;
        }
        synchronized (lock) {
            return poll0();
        }
    }

    public Reference<? extends T> remove(long timeout) throws InterruptedException {
        if (timeout < 0) {
            throw new IllegalArgumentException("Negative timeout value");
        }
        return poll();
    }

    public Reference<? extends T> remove() throws InterruptedException {
        return poll();
    }
}
