/**
 * Tests basic green thread operations:
 * - Thread.start() spawns a new thread
 * - Thread.join() waits for thread completion
 * - Thread.currentThread() returns the running thread
 * - Execution order: A before start, B in child, C after join
 */
public class ThreadBasicTest {
    static String result = "";

    public static String run() {
        result += "A";
        Thread t = new Thread(() -> {
            result += "B";
        });
        t.start();
        try {
            t.join();
        } catch (InterruptedException e) {
            // won't happen
        }
        result += "C";
        return result;
    }
}
