public class ObjectsPredicatesTest {
    public static String run() {
        Object value = "x";
        Object nil = null;
        return java.util.Objects.nonNull(value)
            + "|"
            + java.util.Objects.nonNull(nil)
            + "|"
            + java.util.Objects.isNull(value)
            + "|"
            + java.util.Objects.isNull(nil);
    }
}
