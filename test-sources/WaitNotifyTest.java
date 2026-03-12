/**
 * Tests Object.wait/notify with a simple producer-consumer pattern.
 * - Producer thread produces a value and notifies the consumer.
 * - Consumer thread waits until the value is available.
 * - Result: "produced:42,consumed:42"
 */
public class WaitNotifyTest {
    static final Object lock = new Object();
    static int value = 0;
    static boolean ready = false;

    public static String run() {
        StringBuilder sb = new StringBuilder();

        Thread producer = new Thread(() -> {
            synchronized (lock) {
                value = 42;
                ready = true;
                lock.notify();
            }
        });

        Thread consumer = new Thread(() -> {
            synchronized (lock) {
                while (!ready) {
                    try {
                        lock.wait();
                    } catch (InterruptedException e) {
                        // won't happen
                    }
                }
            }
        });

        // Start consumer first so it waits, then producer notifies.
        consumer.start();
        producer.start();

        try {
            consumer.join();
            producer.join();
        } catch (InterruptedException e) {
            // won't happen
        }

        sb.append("produced:");
        sb.append(value);
        sb.append(",consumed:");
        sb.append(value);
        return sb.toString();
    }
}
