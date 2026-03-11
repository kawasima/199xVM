package java.lang.reflect;

import java.lang.annotation.Annotation;

public final class Constructor<T> extends Executable {
    private final Class<T> clazz;
    private final Class<?>[] parameterTypes;
    private final Class<?>[] exceptionTypes;
    private final int modifiers;

    Constructor(Class<T> declaringClass,
                Class<?>[] parameterTypes,
                Class<?>[] checkedExceptions,
                int modifiers,
                int slot,
                String signature,
                byte[] annotations,
                byte[] parameterAnnotations) {
        this.clazz = declaringClass;
        this.parameterTypes = parameterTypes == null ? new Class<?>[0] : parameterTypes;
        this.exceptionTypes = checkedExceptions == null ? new Class<?>[0] : checkedExceptions;
        this.modifiers = modifiers;
    }

    @Override
    byte[] getAnnotationBytes() {
        return new byte[0];
    }

    @Override
    boolean hasGenericInformation() {
        return false;
    }

    @Override
    String getGenericSignature() {
        return null;
    }

    @Override
    AnnotatedType getAnnotatedReturnType0(Type returnType) {
        return null;
    }

    @Override
    void specificToStringHeader(StringBuilder sb) {
        sb.append(getDeclaringClass().getName());
    }

    @Override
    void specificToGenericStringHeader(StringBuilder sb) {
        specificToStringHeader(sb);
    }

    @Override
    Class<?>[] getSharedParameterTypes() {
        return parameterTypes;
    }

    @Override
    Class<?>[] getSharedExceptionTypes() {
        return exceptionTypes;
    }

    @Override
    public Class<T> getDeclaringClass() {
        return clazz;
    }

    @Override
    public String getName() {
        return getDeclaringClass().getName();
    }

    @Override
    public int getModifiers() {
        return modifiers;
    }

    @Override
    public TypeVariable<?>[] getTypeParameters() {
        return new TypeVariable<?>[0];
    }

    @Override
    public Class<?>[] getParameterTypes() {
        return parameterTypes.clone();
    }

    @Override
    public Class<?>[] getExceptionTypes() {
        return exceptionTypes.clone();
    }

    @Override
    public boolean equals(Object obj) {
        if (!(obj instanceof Constructor)) {
            return false;
        }
        Constructor<?> other = (Constructor<?>) obj;
        return clazz == other.clazz;
    }

    @Override
    public int hashCode() {
        return clazz.getName().hashCode();
    }

    @Override
    public String toString() {
        return getDeclaringClass().getTypeName();
    }

    @Override
    public String toGenericString() {
        return toString();
    }

    public native T newInstance(Object... initargs)
            throws InstantiationException, IllegalAccessException,
                   IllegalArgumentException, InvocationTargetException;

    @Override
    public <T extends Annotation> T getAnnotation(Class<T> annotationClass) {
        Annotation[] annotations = getDeclaredAnnotations();
        for (int i = 0; i < annotations.length; i++) {
            Annotation a = annotations[i];
            if (annotationClass.isInstance(a)) {
                return annotationClass.cast(a);
            }
        }
        return null;
    }

    @Override
    public native Annotation[] getDeclaredAnnotations();

    public AnnotatedType getAnnotatedReturnType() {
        return null;
    }
}
