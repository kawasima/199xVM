package java.lang.invoke;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

public final class MethodHandle {
    private final Method method;

    MethodHandle(Method method) {
        if (method == null) {
            throw new NullPointerException("method");
        }
        this.method = method;
    }

    private Object invokeInternal(Object receiver, Object[] args) throws Throwable {
        try {
            return method.invoke(receiver, args);
        } catch (InvocationTargetException e) {
            if (e.getCause() != null) {
                throw e.getCause();
            }
            throw e;
        }
    }

    public Object invoke(Object receiver) throws Throwable {
        return invokeInternal(receiver, new Object[0]);
    }

    public Object invoke(Object receiver, Object arg0) throws Throwable {
        return invokeInternal(receiver, new Object[] { arg0 });
    }

    public boolean invoke(Method receiver, Object arg0) throws Throwable {
        Object result = invokeInternal(receiver, new Object[] { arg0 });
        return result != null && ((Boolean) result).booleanValue();
    }
}
