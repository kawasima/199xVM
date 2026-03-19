import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.reflect.Method;
import java.util.ArrayList;

public class MethodHandleShimTest {
    public static String run() throws Throwable {
        MethodHandle canAccess = MethodHandles.lookup()
                .findVirtual(Method.class, "canAccess", MethodType.methodType(boolean.class, Object.class));

        Method sample = MethodHandleShimTest.class.getMethod("sample", String.class);
        if (!(boolean) canAccess.invoke(sample, (Object) null)) {
            return "FAIL:canAccess";
        }

        String noArgs = MethodType.methodType(void.class).toMethodDescriptorString();
        String arrayArgs = MethodType.methodType(String.class, new Class<?>[] { int.class, Object[].class })
                .toMethodDescriptorString();

        ArrayList<Class<?>> listArgs = new ArrayList<>();
        listArgs.add(Object.class);
        listArgs.add(long.class);
        String listDesc = MethodType.methodType(int.class, listArgs).toMethodDescriptorString();

        return "canAccess|" + noArgs + "|" + arrayArgs + "|" + listDesc;
    }

    public static String sample(String value) {
        return value;
    }
}
