package java.lang.reflect;

import java.lang.annotation.Annotation;

public final class Parameter implements AnnotatedElement {
    private final String name;
    private final int modifiers;
    private final Executable executable;
    private final int index;

    Parameter(String name, int modifiers, Executable executable, int index) {
        this.name = name;
        this.modifiers = modifiers;
        this.executable = executable;
        this.index = index;
    }

    public boolean equals(Object obj) {
        if (!(obj instanceof Parameter)) {
            return false;
        }
        Parameter other = (Parameter) obj;
        return executable.equals(other.executable) && index == other.index;
    }

    public int hashCode() {
        return executable.hashCode() ^ index;
    }

    public boolean isNamePresent() {
        return name != null;
    }

    public String toString() {
        return getType().getTypeName() + " " + getName();
    }

    public Executable getDeclaringExecutable() {
        return executable;
    }

    public int getModifiers() {
        return modifiers;
    }

    public String getName() {
        return name == null ? "arg" + index : name;
    }

    public Type getParameterizedType() {
        Type[] types = executable.getGenericParameterTypes();
        return index < types.length ? types[index] : Object.class;
    }

    public Class<?> getType() {
        Class<?>[] types = executable.getParameterTypes();
        return index < types.length ? types[index] : Object.class;
    }

    public AnnotatedType getAnnotatedType() {
        return null;
    }

    public boolean isImplicit() {
        return false;
    }

    public boolean isSynthetic() {
        return Modifier.isSynthetic(modifiers);
    }

    public boolean isVarArgs() {
        return executable.isVarArgs() && index == executable.getParameterCount() - 1;
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
    public Annotation[] getAnnotations() {
        return getDeclaredAnnotations();
    }

    @Override
    public <T extends Annotation> T[] getAnnotationsByType(Class<T> annotationClass) {
        return getDeclaredAnnotationsByType(annotationClass);
    }

    @Override
    public Annotation[] getDeclaredAnnotations() {
        Annotation[][] all = executable.getParameterAnnotations();
        if (index < 0 || index >= all.length) {
            return new Annotation[0];
        }
        return all[index];
    }
}
