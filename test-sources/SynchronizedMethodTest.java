/**
 * Test ACC_SYNCHRONIZED methods: instance and static synchronized methods
 * should behave like synchronized blocks, providing mutual exclusion.
 */
public class SynchronizedMethodTest {
    static int counter = 0;

    public synchronized void increment() {
        int tmp = counter;
        counter = tmp + 1;
    }

    public static synchronized void staticIncrement() {
        int tmp = counter;
        counter = tmp + 1;
    }

    /** Synchronized method that throws — monitor must be released on exception. */
    public synchronized void throwingIncrement() {
        counter++;
        throw new RuntimeException("boom");
    }

    public static String run() {
        SynchronizedMethodTest obj = new SynchronizedMethodTest();

        // Test instance synchronized method (single-threaded)
        counter = 0;
        obj.increment();
        obj.increment();
        obj.increment();
        String r1 = String.valueOf(counter);

        // Test static synchronized method (single-threaded)
        counter = 0;
        staticIncrement();
        staticIncrement();
        String r2 = String.valueOf(counter);

        // Test that synchronized methods provide mutual exclusion across threads
        counter = 0;
        Thread t1 = new Thread(() -> {
            for (int i = 0; i < 100; i++) {
                obj.increment();
            }
        });
        Thread t2 = new Thread(() -> {
            for (int i = 0; i < 100; i++) {
                obj.increment();
            }
        });
        t1.start();
        t2.start();
        try {
            t1.join();
            t2.join();
        } catch (InterruptedException e) {
        }
        String r3 = String.valueOf(counter);

        // Test that monitor is released when synchronized method throws
        counter = 0;
        try {
            obj.throwingIncrement();
        } catch (RuntimeException e) {
            // expected
        }
        // If monitor was NOT released, this would deadlock
        obj.increment();
        String r4 = String.valueOf(counter);

        return r1 + "," + r2 + "," + r3 + "," + r4;
    }
}
