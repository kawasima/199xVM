public class ReflectionInvocationCauseTest {
    public static class Boom extends Exception {
        public Boom(String message) {
            super(message);
        }
    }

    public static class Fixture {
        public static void fail() throws Boom {
            throw new Boom("boom");
        }
    }

    public static String run() {
        try {
            java.lang.reflect.Method method = Fixture.class.getMethod("fail");
            method.invoke(null);
            return "no-throw";
        } catch (java.lang.reflect.InvocationTargetException e) {
            Throwable cause = e.getCause();
            if (cause == null) {
                return "java.lang.reflect.InvocationTargetException|null";
            }
            return e.getClass().getName() + "|" + cause.getClass().getName() + ":" + cause.getMessage();
        } catch (Exception e) {
            return "other:" + e.getClass().getName();
        }
    }
}
