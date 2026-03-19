package java.lang.invoke;

import java.io.Serializable;
import java.util.List;

public final class MethodType implements Serializable {
    private final Class<?> returnType;
    private final Class<?>[] parameterTypes;

    private MethodType(Class<?> returnType, Class<?>[] parameterTypes) {
        if (returnType == null) {
            throw new NullPointerException("returnType");
        }
        this.returnType = returnType;
        this.parameterTypes = cloneTypes(parameterTypes);
    }

    public static MethodType methodType(Class<?> returnType) {
        return new MethodType(returnType, new Class<?>[0]);
    }

    public static MethodType methodType(Class<?> returnType, Class<?> parameterType) {
        return new MethodType(returnType, new Class<?>[] { parameterType });
    }

    public static MethodType methodType(Class<?> returnType, Class<?>[] parameterTypes) {
        return new MethodType(returnType, parameterTypes);
    }

    @SuppressWarnings("rawtypes")
    public static MethodType methodType(Class<?> returnType, List parameterTypes) {
        if (parameterTypes == null) {
            return new MethodType(returnType, null);
        }
        Class<?>[] out = new Class<?>[parameterTypes.size()];
        for (int i = 0; i < out.length; i++) {
            out[i] = (Class<?>) parameterTypes.get(i);
        }
        return new MethodType(returnType, out);
    }

    Class<?>[] parameterArray() {
        return cloneTypes(parameterTypes);
    }

    public String toMethodDescriptorString() {
        StringBuilder out = new StringBuilder();
        out.append('(');
        for (int i = 0; i < parameterTypes.length; i++) {
            appendDescriptor(out, parameterTypes[i]);
        }
        out.append(')');
        appendDescriptor(out, returnType);
        return out.toString();
    }

    private static Class<?>[] cloneTypes(Class<?>[] source) {
        if (source == null) {
            return new Class<?>[0];
        }
        Class<?>[] out = new Class<?>[source.length];
        for (int i = 0; i < source.length; i++) {
            if (source[i] == null) {
                throw new NullPointerException("parameterType");
            }
            out[i] = source[i];
        }
        return out;
    }

    private static void appendDescriptor(StringBuilder out, Class<?> type) {
        if (type == null) {
            throw new NullPointerException("type");
        }
        if (type.isArray()) {
            out.append(type.getName().replace('.', '/'));
            return;
        }
        if (type.isPrimitive()) {
            if (type == Void.TYPE) {
                out.append('V');
            } else if (type == Boolean.TYPE) {
                out.append('Z');
            } else if (type == Byte.TYPE) {
                out.append('B');
            } else if (type == Character.TYPE) {
                out.append('C');
            } else if (type == Short.TYPE) {
                out.append('S');
            } else if (type == Integer.TYPE) {
                out.append('I');
            } else if (type == Long.TYPE) {
                out.append('J');
            } else if (type == Float.TYPE) {
                out.append('F');
            } else if (type == Double.TYPE) {
                out.append('D');
            } else {
                throw new IllegalArgumentException("unknown primitive type: " + type);
            }
            return;
        }
        out.append('L');
        out.append(type.getName().replace('.', '/'));
        out.append(';');
    }
}
