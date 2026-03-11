package java.util.concurrent.locks;

import java.io.Serializable;
import java.util.Date;
import java.util.concurrent.TimeUnit;

public class ReentrantLock implements Lock, Serializable {
    private static final long serialVersionUID = 7373984872572414699L;

    public ReentrantLock() {}
    public ReentrantLock(boolean fair) {}

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

    public int getHoldCount() {
        return 0;
    }

    public boolean isHeldByCurrentThread() {
        return false;
    }

    public boolean isLocked() {
        return false;
    }

    public final boolean isFair() {
        return false;
    }

    public final boolean hasQueuedThreads() {
        return false;
    }

    public final int getQueueLength() {
        return 0;
    }

    public String toString() {
        return super.toString();
    }
}
