public class ClinitExceptionTest {
    static class BadInit {
        static String VALUE = initValue();
        static String initValue() {
            throw new RuntimeException("clinit failed");
        }
    }

    public static String run() {
        try {
            // Accessing BadInit forces <clinit> to run and throw.
            return BadInit.VALUE;
        } catch (ExceptionInInitializerError e) {
            return "ExceptionInInitializerError";
        } catch (Throwable t) {
            return "other:" + t.getClass().getSimpleName();
        }
    }
}
