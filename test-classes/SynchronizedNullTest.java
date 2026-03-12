/**
 * Tests that synchronized(null) throws NullPointerException.
 */
public class SynchronizedNullTest {
    public static String run() {
        try {
            Object obj = null;
            synchronized (obj) {
                return "should-not-reach";
            }
        } catch (NullPointerException e) {
            return "npe-ok";
        }
    }
}
