public class InterfaceMethodResolutionChoiceTest {
    interface IgnoredStaticMethod {
        static String value() {
            return "static";
        }
    }

    interface IgnoredPrivateMethod {
        private String value() {
            return "private";
        }
    }

    interface DefaultAgainstStaticMethod {
        default String value() {
            return "ignored-static";
        }
    }

    interface DefaultAgainstPrivateMethod {
        default String value() {
            return "ignored-private";
        }
    }

    interface StaticCombined extends IgnoredStaticMethod, DefaultAgainstStaticMethod {}

    interface PrivateCombined extends IgnoredPrivateMethod, DefaultAgainstPrivateMethod {}

    static final class StaticImpl implements StaticCombined {}

    static final class PrivateImpl implements PrivateCombined {}

    public static String run() {
        StaticCombined staticCombined = new StaticImpl();
        PrivateCombined privateCombined = new PrivateImpl();
        return staticCombined.value() + "|" + privateCombined.value();
    }
}
