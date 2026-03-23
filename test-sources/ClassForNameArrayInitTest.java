public class ClassForNameArrayInitTest {
    public static class InitCounter {
        static int count = 0;
        static {
            count++;
        }
    }

    public static String run() throws Exception {
        Class<?> arrayClass = Class.forName("[Ljava.lang.String;", true, ClassLoader.getSystemClassLoader());
        Class.forName("ClassForNameArrayInitTest$InitCounter", true, ClassLoader.getSystemClassLoader());
        Class.forName("ClassForNameArrayInitTest$InitCounter", true, ClassLoader.getSystemClassLoader());
        return (arrayClass != null ? "array-ok" : "array-null") + "|" + InitCounter.count;
    }
}
