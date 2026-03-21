public class ThreadStackTraceShimTest {
    public static String run() {
        StackTraceElement[] trace = Thread.currentThread().getStackTrace();
        if (trace == null) {
            return "null";
        }
        return "len=" + trace.length;
    }
}
