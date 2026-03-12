/**
 * Tests that calling wait/notify without holding the monitor
 * throws IllegalMonitorStateException.
 */
public class WaitWithoutLockTest {
    public static String run() {
        Object lock = new Object();
        StringBuilder sb = new StringBuilder();

        // wait() without lock
        try {
            lock.wait();
            sb.append("FAIL");
        } catch (IllegalMonitorStateException e) {
            sb.append("wait:IMSE");
        } catch (InterruptedException e) {
            sb.append("FAIL");
        }

        sb.append(",");

        // notify() without lock
        try {
            lock.notify();
            sb.append("FAIL");
        } catch (IllegalMonitorStateException e) {
            sb.append("notify:IMSE");
        }

        sb.append(",");

        // notifyAll() without lock
        try {
            lock.notifyAll();
            sb.append("FAIL");
        } catch (IllegalMonitorStateException e) {
            sb.append("notifyAll:IMSE");
        }

        return sb.toString();
    }
}
