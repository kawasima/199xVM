public class VirtualMonomorphicDispatchTest {
    abstract static class Base {
        abstract String label();
    }

    static final class A extends Base {
        @Override
        String label() {
            return "A";
        }
    }

    static final class B extends Base {
        @Override
        String label() {
            return "B";
        }
    }

    private static String call(Base value) {
        return value.label();
    }

    public static String run() {
        return call(new A()) + "|" + call(new B()) + "|" + call(new A());
    }
}
