/**
 * Tests notifyAll() waking multiple waiting threads.
 * Three consumer threads wait on a shared lock. Producer calls notifyAll().
 * Result: "3" (all three consumers woken).
 */
public class NotifyAllTest {
    static final Object lock = new Object();
    static volatile int wokenCount = 0;
    static volatile boolean go = false;

    public static String run() {
        Thread c1 = new Thread(() -> waitForGo());
        Thread c2 = new Thread(() -> waitForGo());
        Thread c3 = new Thread(() -> waitForGo());

        c1.start();
        c2.start();
        c3.start();

        // Give consumers time to enter wait (yield a few times).
        Thread.yield();
        Thread.yield();
        Thread.yield();

        synchronized (lock) {
            go = true;
            lock.notifyAll();
        }

        try {
            c1.join();
            c2.join();
            c3.join();
        } catch (InterruptedException e) {}

        return String.valueOf(wokenCount);
    }

    static void waitForGo() {
        synchronized (lock) {
            while (!go) {
                try {
                    lock.wait();
                } catch (InterruptedException e) {}
            }
            wokenCount++;
        }
    }
}
