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

package java.util.concurrent.locks;

import java.io.Serializable;
import java.util.concurrent.TimeUnit;

/**
 * An implementation of {@link ReadWriteLock} that supports similar
 * semantics to {@link ReentrantLock}.
 *
 * <p>199xVM shim: simplified implementation using synchronized blocks.
 * The VM supports threads with monitorenter/monitorexit, so this
 * implementation is thread-safe via intrinsic locks.
 *
 * @since 1.5
 * @author Doug Lea
 */
public class ReentrantReadWriteLock implements ReadWriteLock, Serializable {

    private final ReadLock readerLock;
    private final WriteLock writerLock;
    private final Object sync = new Object();
    private int readCount = 0;
    private int writeCount = 0;
    private Thread writeOwner = null;

    public ReentrantReadWriteLock() {
        this(false);
    }

    public ReentrantReadWriteLock(boolean fair) {
        readerLock = new ReadLock(this);
        writerLock = new WriteLock(this);
    }

    public ReadLock readLock() { return readerLock; }
    public WriteLock writeLock() { return writerLock; }

    public final boolean isFair() { return false; }

    public int getReadLockCount() {
        synchronized (sync) { return readCount; }
    }

    public boolean isWriteLocked() {
        synchronized (sync) { return writeCount > 0; }
    }

    public boolean isWriteLockedByCurrentThread() {
        synchronized (sync) { return writeOwner == Thread.currentThread() && writeCount > 0; }
    }

    public int getWriteHoldCount() {
        synchronized (sync) {
            return (writeOwner == Thread.currentThread()) ? writeCount : 0;
        }
    }

    public int getReadHoldCount() {
        synchronized (sync) { return readCount; }
    }

    public static class ReadLock implements Lock, Serializable {
        private final ReentrantReadWriteLock outer;

        protected ReadLock(ReentrantReadWriteLock lock) {
            this.outer = lock;
        }

        public void lock() {
            synchronized (outer.sync) {
                outer.readCount++;
            }
        }

        public void lockInterruptibly() throws InterruptedException {
            lock();
        }

        public boolean tryLock() {
            lock();
            return true;
        }

        public boolean tryLock(long timeout, TimeUnit unit) throws InterruptedException {
            lock();
            return true;
        }

        public void unlock() {
            synchronized (outer.sync) {
                if (outer.readCount > 0) outer.readCount--;
            }
        }

        public Condition newCondition() {
            throw new UnsupportedOperationException();
        }

        public String toString() {
            return super.toString() + "[Read locks = " + outer.getReadLockCount() + "]";
        }
    }

    public static class WriteLock implements Lock, Serializable {
        private final ReentrantReadWriteLock outer;

        protected WriteLock(ReentrantReadWriteLock lock) {
            this.outer = lock;
        }

        public void lock() {
            synchronized (outer.sync) {
                outer.writeCount++;
                outer.writeOwner = Thread.currentThread();
            }
        }

        public void lockInterruptibly() throws InterruptedException {
            lock();
        }

        public boolean tryLock() {
            lock();
            return true;
        }

        public boolean tryLock(long timeout, TimeUnit unit) throws InterruptedException {
            lock();
            return true;
        }

        public void unlock() {
            synchronized (outer.sync) {
                if (outer.writeCount > 0) {
                    outer.writeCount--;
                    if (outer.writeCount == 0) {
                        outer.writeOwner = null;
                    }
                }
            }
        }

        public Condition newCondition() {
            throw new UnsupportedOperationException();
        }

        public boolean isHeldByCurrentThread() {
            return outer.isWriteLockedByCurrentThread();
        }

        public int getHoldCount() {
            return outer.getWriteHoldCount();
        }

        public String toString() {
            Thread o = outer.writeOwner;
            return super.toString() + ((o == null) ?
                "[Unlocked]" :
                "[Locked by thread " + o.getName() + "]");
        }
    }

    public String toString() {
        return super.toString() + "[Write locks = " + getWriteHoldCount() +
            ", Read locks = " + getReadLockCount() + "]";
    }
}
