package java.lang.reflect;

import java.lang.annotation.Annotation;

public interface AnnotatedElement {
    default boolean isAnnotationPresent(Class<? extends Annotation> annotationClass) {
        return getAnnotation(annotationClass) != null;
    }

    default <T extends Annotation> T getAnnotation(Class<T> annotationClass) {
        Annotation[] annotations = getAnnotations();
        for (int i = 0; i < annotations.length; i++) {
            Annotation annotation = annotations[i];
            if (annotationClass.isInstance(annotation)) {
                return annotationClass.cast(annotation);
            }
        }
        return null;
    }

    Annotation[] getAnnotations();

    default <T extends Annotation> T[] getAnnotationsByType(Class<T> annotationClass) {
        return getDeclaredAnnotationsByType(annotationClass);
    }

    default <T extends Annotation> T getDeclaredAnnotation(Class<T> annotationClass) {
        Annotation[] annotations = getDeclaredAnnotations();
        for (int i = 0; i < annotations.length; i++) {
            Annotation annotation = annotations[i];
            if (annotationClass.isInstance(annotation)) {
                return annotationClass.cast(annotation);
            }
        }
        return null;
    }

    @SuppressWarnings("unchecked")
    default <T extends Annotation> T[] getDeclaredAnnotationsByType(Class<T> annotationClass) {
        T annotation = getDeclaredAnnotation(annotationClass);
        if (annotation == null) {
            return (T[]) new Annotation[0];
        }
        return (T[]) new Annotation[] { annotation };
    }

    Annotation[] getDeclaredAnnotations();
}
