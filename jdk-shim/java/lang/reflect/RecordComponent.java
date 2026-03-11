package java.lang.reflect;

import java.lang.annotation.Annotation;

public final class RecordComponent implements AnnotatedElement {
    private final Class<?> clazz;
    private final String name;
    private final Class<?> type;

    RecordComponent(Class<?> declaringClass, String name, Class<?> type) {
        this.clazz = declaringClass;
        this.name = name;
        this.type = type;
    }

    public String getName() {
        return name;
    }

    public Class<?> getType() {
        return type;
    }

    public Type getGenericType() {
        return type;
    }

    public Class<?> getDeclaringRecord() {
        return clazz;
    }

    public Method getAccessor() {
        return null;
    }

    public String getSignature() {
        return null;
    }

    public AnnotatedType getAnnotatedType() {
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
    public Annotation[] getAnnotations() {
        return getDeclaredAnnotations();
    }

    @Override
    public native Annotation[] getDeclaredAnnotations();

    @Override
    public String toString() {
        return type.getTypeName() + " " + name;
    }
}
