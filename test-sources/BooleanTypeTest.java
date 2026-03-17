public class BooleanTypeTest {
    public static String run() {
        // This triggers Boolean.<clinit> which calls Class.getPrimitiveClass("boolean")
        Class<?> t = Boolean.TYPE;
        return t.getName();
    }
}
