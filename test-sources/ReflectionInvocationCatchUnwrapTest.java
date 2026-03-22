public class ReflectionInvocationCatchUnwrapTest {
    public static class Cookies extends Exception {
        public Cookies(String message) {
            super(message);
        }
    }

    public static void fail() throws Cookies {
        throw new Cookies("wrapped");
    }

    private static Throwable getCauseOrElse(Exception e) {
        return e.getCause() != null ? e.getCause() : e;
    }

    public static String run() {
        try {
            try {
                java.lang.reflect.Method method =
                    ReflectionInvocationCatchUnwrapTest.class.getMethod("fail");
                method.invoke(null);
                return "no-throw";
            } catch (Exception e) {
                Throwable throwable = getCauseOrElse(e);
                if (throwable instanceof Cookies) {
                    throw (Cookies) throwable;
                }
                throw new RuntimeException(throwable);
            }
        } catch (Cookies cookies) {
            return cookies.getClass().getName() + ":" + cookies.getMessage();
        } catch (Throwable throwable) {
            return "outer:" + throwable.getClass().getName() + ":" + throwable.getMessage();
        }
    }
}
