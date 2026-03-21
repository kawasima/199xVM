import java.lang.reflect.Method;

public class ReflectionInterfaceDefaultMethodTest {
    interface Named {
        default String describe() {
            return "iface";
        }
    }

    interface ExtendedNamed extends Named {}

    static final class NamedImpl implements ExtendedNamed {}

    public static String run() throws Exception {
        Method method = NamedImpl.class.getMethod("describe");
        String value = (String) method.invoke(new NamedImpl());
        boolean listed = false;
        for (Method candidate : NamedImpl.class.getMethods()) {
            if ("describe".equals(candidate.getName()) && candidate.getParameterCount() == 0) {
                listed = true;
                break;
            }
        }
        return value + "|" + listed;
    }
}
