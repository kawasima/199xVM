/**
 * Tests monitor contention between two green threads.
 * Two threads increment a shared counter inside a synchronized block.
 * The monitor ensures mutual exclusion so the final count is correct.
 */
public class MonitorContentionTest {
    static int counter = 0;
    static final Object lock = new Object();

    public static String run() {
        Thread t1 = new Thread(() -> {
            for (int i = 0; i < 5; i++) {
                synchronized (lock) {
                    counter++;
                }
            }
        });
        Thread t2 = new Thread(() -> {
            for (int i = 0; i < 5; i++) {
                synchronized (lock) {
                    counter++;
                }
            }
        });
        t1.start();
        t2.start();
        try {
            t1.join();
            t2.join();
        } catch (InterruptedException e) {
            // won't happen
        }
        return String.valueOf(counter);
    }
}
