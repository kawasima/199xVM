/**
 * Tests that wait() correctly saves and restores reentrant monitor count.
 * Thread acquires the same lock 3 times (count=3), then calls wait().
 * After notify, the reentrant count should be restored to 3.
 * Result: "ok" if reentrant count is properly restored (no deadlock/hang).
 */
public class ReentrantWaitTest {
    static final Object lock = new Object();
    static volatile boolean ready = false;

    public static String run() {
        Thread waiter = new Thread(() -> {
            synchronized (lock) {
                synchronized (lock) {
                    synchronized (lock) {
                        // count = 3 at this point
                        while (!ready) {
                            try {
                                lock.wait();
                            } catch (InterruptedException e) {}
                        }
                    }
                    // count should be 2 here (after exiting inner sync)
                }
                // count should be 1 here
            }
            // fully released
        });

        waiter.start();

        // Let waiter enter wait()
        Thread.yield();
        Thread.yield();

        synchronized (lock) {
            ready = true;
            lock.notify();
        }

        try {
            waiter.join();
        } catch (InterruptedException e) {}

        return "ok";
    }
}
