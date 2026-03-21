import java.lang.reflect.Method;

public class ReflectionMethodArrayIsolationTest {
    interface Named {
        default String describe() {
            return "named";
        }
    }

    static class NamedImpl implements Named {
        public void alpha() {}

        public void beta() {}
    }

    private static boolean allNonNull(Method[] methods) {
        for (int i = 0; i < methods.length; i++) {
            if (methods[i] == null) {
                return false;
            }
        }
        return true;
    }

    public static String run() {
        Method[] declaredFirst = NamedImpl.class.getDeclaredMethods();
        declaredFirst[0] = null;
        Method[] declaredSecond = NamedImpl.class.getDeclaredMethods();

        Method[] publicFirst = NamedImpl.class.getMethods();
        publicFirst[0] = null;
        Method[] publicSecond = NamedImpl.class.getMethods();

        boolean hasDefault = false;
        for (int i = 0; i < publicSecond.length; i++) {
            if ("describe".equals(publicSecond[i].getName())) {
                hasDefault = true;
                break;
            }
        }

        return allNonNull(declaredSecond)
            + "|"
            + allNonNull(publicSecond)
            + "|"
            + hasDefault;
    }
}
