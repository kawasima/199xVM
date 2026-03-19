package java.lang.invoke;

import java.lang.reflect.Method;

public final class MethodHandles {
    private MethodHandles() {}

    public static Lookup lookup() {
        return new Lookup();
    }

    public static final class Lookup {
        Lookup() {}

        public MethodHandle findVirtual(Class<?> refc, String name, MethodType type)
                throws NoSuchMethodException, IllegalAccessException {
            Method method = refc.getMethod(name, type.parameterArray());
            return new MethodHandle(method);
        }
    }
}
