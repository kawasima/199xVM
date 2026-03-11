package java.lang.reflect;

import java.lang.annotation.Annotation;

public class AccessibleObject implements AnnotatedElement {
    boolean override;

    protected AccessibleObject() {}

    public static void setAccessible(AccessibleObject[] array, boolean flag)
            throws SecurityException {
        if (array == null) {
            return;
        }
        for (int i = 0; i < array.length; i++) {
            if (array[i] != null) {
                array[i].setAccessible(flag);
            }
        }
    }

    public void setAccessible(boolean flag) throws SecurityException {
        override = flag;
    }

    public final boolean trySetAccessible() {
        override = true;
        return true;
    }

    @Deprecated
    public boolean isAccessible() {
        return override;
    }

    public final boolean canAccess(Object obj) {
        return true;
    }

    void checkCanSetAccessible(Class<?> caller) {}

    final void checkAccess(Class<?> caller, Class<?> memberClass, Class<?> targetClass, int modifiers)
            throws IllegalAccessException {
    }

    @Override
    public <T extends Annotation> T getAnnotation(Class<T> annotationClass) {
        return null;
    }

    @Override
    public Annotation[] getAnnotations() {
        return new Annotation[0];
    }

    @Override
    public Annotation[] getDeclaredAnnotations() {
        return new Annotation[0];
    }
}
