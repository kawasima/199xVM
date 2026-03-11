package java.lang.reflect;

import java.lang.annotation.Annotation;

public final class Method extends Executable {
    private final Class<?> clazz;
    private final String name;
    private final Class<?> returnType;
    private final Class<?>[] parameterTypes;
    private final Class<?>[] exceptionTypes;
    private final int modifiers;

    Method(Class<?> declaringClass,
           String name,
           Class<?>[] parameterTypes,
           Class<?> returnType,
           Class<?>[] checkedExceptions,
           int modifiers,
           int slot,
           String signature,
           byte[] annotations,
           byte[] parameterAnnotations,
           byte[] annotationDefault) {
        this.clazz = declaringClass;
        this.name = name;
        this.returnType = returnType;
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
        sb.append(getDeclaringClass().getName()).append('.').append(getName());
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
    public Class<?> getDeclaringClass() {
        return clazz;
    }

    @Override
    public String getName() {
        return name;
    }

    @Override
    public int getModifiers() {
        return modifiers;
    }

    @Override
    public TypeVariable<?>[] getTypeParameters() {
        return new TypeVariable<?>[0];
    }

    public Class<?> getReturnType() {
        return returnType;
    }

    public Type getGenericReturnType() {
        return returnType;
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
        if (!(obj instanceof Method)) {
            return false;
        }
        Method other = (Method) obj;
        return clazz == other.clazz && name.equals(other.name);
    }

    @Override
    public int hashCode() {
        return clazz.getName().hashCode() ^ name.hashCode();
    }

    @Override
    public String toString() {
        return getDeclaringClass().getTypeName() + "." + getName();
    }

    @Override
    public String toGenericString() {
        return toString();
    }

    public native Object invoke(Object obj, Object... args)
            throws IllegalAccessException, IllegalArgumentException, InvocationTargetException;

    public boolean isBridge() {
        return (getModifiers() & Modifier.BRIDGE) != 0;
    }

    public boolean isDefault() {
        return false;
    }

    public boolean isVarArgs() {
        return super.isVarArgs();
    }

    public Object getDefaultValue() {
        return null;
    }

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
