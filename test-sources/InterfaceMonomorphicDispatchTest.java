public class InterfaceMonomorphicDispatchTest {
    interface Named {
        String label();
    }

    static final class A implements Named {
        @Override
        public String label() {
            return "A";
        }
    }

    static final class B implements Named {
        @Override
        public String label() {
            return "B";
        }
    }

    private static String call(Named value) {
        return value.label();
    }

    public static String run() {
        return call(new A()) + "|" + call(new B()) + "|" + call(new A());
    }
}
