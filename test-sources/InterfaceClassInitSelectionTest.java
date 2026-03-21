public class InterfaceClassInitSelectionTest {
    static class Log {
        static StringBuilder value = new StringBuilder();

        static String mark(String s) {
            value.append(s);
            return s;
        }
    }

    interface Plain {
        String MARK = Log.mark("P");
    }

    interface WithConcreteMethod {
        String MARK = Log.mark("D");

        default String describe() {
            return "ok";
        }
    }

    static class Impl implements Plain, WithConcreteMethod {
    }

    public static String run() {
        new Impl();
        return Log.value.toString();
    }
}
