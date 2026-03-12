public class ClinitErroneousStateTest {
    static class BadInit {
        static String VALUE = initValue();
        static String initValue() {
            throw new RuntimeException("clinit failed");
        }
    }

    public static String run() {
        String first;
        try {
            first = BadInit.VALUE;
        } catch (ExceptionInInitializerError e) {
            first = "EIIE";
        } catch (Throwable t) {
            first = "other1:" + t.getClass().getSimpleName();
        }

        // Second access to the same erroneous class must throw NoClassDefFoundError
        // per JVMS §5.5 (class is in erroneous state after failed <clinit>).
        String second;
        try {
            second = BadInit.VALUE;
        } catch (NoClassDefFoundError e) {
            second = "NCDFE";
        } catch (Throwable t) {
            second = "other2:" + t.getClass().getSimpleName();
        }

        return first + "," + second;
    }
}
