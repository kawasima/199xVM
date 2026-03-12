/**
 * Tests that synchronized blocks work correctly.
 * Exercises monitorenter/monitorexit with reentrant locking.
 */
public class SynchronizedTest {
    static int counter = 0;

    public static String run() {
        Object lock = new Object();

        // Basic synchronized block
        synchronized (lock) {
            counter = 1;
        }

        // Reentrant: nested synchronized on same object
        synchronized (lock) {
            synchronized (lock) {
                counter = 2;
            }
            // Still holds outer lock here
            counter = 3;
        }

        // Synchronized on a different object
        Object lock2 = new Object();
        synchronized (lock2) {
            counter += 10;
        }

        return String.valueOf(counter);
    }
}
