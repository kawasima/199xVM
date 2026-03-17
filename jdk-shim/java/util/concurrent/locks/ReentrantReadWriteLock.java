/*
 * Copyright (c) 2003, 2025, Oracle and/or its affiliates. All rights reserved.
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
import java.util.Date;
import java.util.concurrent.TimeUnit;

public class ReentrantReadWriteLock implements Serializable {
    private static final long serialVersionUID = -6992448646407690164L;

    private final ReadLock readerLock;
    private final WriteLock writerLock;

    public ReentrantReadWriteLock() {
        this(false);
    }

    public ReentrantReadWriteLock(boolean fair) {
        this.readerLock = new ReadLock(this);
        this.writerLock = new WriteLock(this);
    }

    public WriteLock writeLock() {
        return writerLock;
    }

    public ReadLock readLock() {
        return readerLock;
    }

    public boolean isFair() {
        return false;
    }

    public int getReadLockCount() {
        return 0;
    }

    public boolean isWriteLocked() {
        return false;
    }

    private static Condition newCondition0() {
        return new Condition() {
            public void await() throws InterruptedException {}
            public void awaitUninterruptibly() {}
            public long awaitNanos(long nanosTimeout) throws InterruptedException { return 0L; }
            public boolean await(long time, TimeUnit unit) throws InterruptedException { return true; }
            public boolean awaitUntil(Date deadline) throws InterruptedException { return true; }
            public void signal() {}
            public void signalAll() {}
        };
    }

    public static class ReadLock implements Lock, Serializable {
        private static final long serialVersionUID = -5992448646407690164L;
        private final ReentrantReadWriteLock lock;

        protected ReadLock(ReentrantReadWriteLock lock) {
            this.lock = lock;
        }

        public void lock() {}

        public void lockInterruptibly() throws InterruptedException {}

        public boolean tryLock() {
            return true;
        }

        public boolean tryLock(long time, TimeUnit unit) throws InterruptedException {
            return true;
        }

        public void unlock() {}

        public Condition newCondition() {
            return ReentrantReadWriteLock.newCondition0();
        }

        public String toString() {
            return super.toString();
        }
    }

    public static class WriteLock implements Lock, Serializable {
        private static final long serialVersionUID = -4992448646407690164L;
        private final ReentrantReadWriteLock lock;

        protected WriteLock(ReentrantReadWriteLock lock) {
            this.lock = lock;
        }

        public void lock() {}

        public void lockInterruptibly() throws InterruptedException {}

        public boolean tryLock() {
            return true;
        }

        public boolean tryLock(long time, TimeUnit unit) throws InterruptedException {
            return true;
        }

        public void unlock() {}

        public Condition newCondition() {
            return ReentrantReadWriteLock.newCondition0();
        }

        public String toString() {
            return super.toString();
        }
    }
}
