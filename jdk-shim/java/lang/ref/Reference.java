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
 * Minimal shim implementation of {@code java.lang.ref.Reference}.
 *
 * <p>The field layout intentionally stays close to the JDK so a future GC can
 * recognize reference objects, treat {@code referent} specially, and reuse the
 * existing queue transitions without changing the public API.
 */
public abstract class Reference<T> {
    private T referent; /* Treated specially by a future GC */

    volatile ReferenceQueue<? super T> queue;

    @SuppressWarnings("rawtypes")
    volatile Reference next;

    private transient Reference<?> discovered;

    public T get() {
        return referent;
    }

    public final boolean refersTo(T obj) {
        return refersToImpl(obj);
    }

    boolean refersToImpl(T obj) {
        return referent == obj;
    }

    public void clear() {
        clearImpl();
    }

    void clearImpl() {
        referent = null;
    }

    @Deprecated(since = "16")
    public boolean isEnqueued() {
        return queue == ReferenceQueue.ENQUEUED;
    }

    public boolean enqueue() {
        return queue.enqueue(this);
    }

    @Override
    protected Object clone() throws CloneNotSupportedException {
        throw new CloneNotSupportedException();
    }

    Reference(T referent) {
        this(referent, null);
    }

    Reference(T referent, ReferenceQueue<? super T> queue) {
        this.referent = referent;
        this.queue = (queue == null) ? ReferenceQueue.NULL_QUEUE : queue;
        this.next = null;
        this.discovered = null;
    }

    public static void reachabilityFence(Object ref) {
        // No-op in the current VM. The public hook remains so GC-aware
        // implementations can tighten semantics later without an API change.
    }
}
