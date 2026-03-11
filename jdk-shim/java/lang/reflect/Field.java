package java.lang.reflect;

import java.lang.annotation.Annotation;

public final class Field extends AccessibleObject implements Member {
    private final Class<?> clazz;
    private final String name;
    private final Class<?> type;
    private final int modifiers;

    Field(Class<?> declaringClass,
          String name,
          Class<?> type,
          int modifiers,
          int slot,
          String signature,
          byte[] annotations) {
        this.clazz = declaringClass;
        this.name = name;
        this.type = type;
        this.modifiers = modifiers;
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
    public boolean isSynthetic() {
        return Modifier.isSynthetic(getModifiers());
    }

    public boolean isEnumConstant() {
        return (getModifiers() & Modifier.ENUM) != 0;
    }

    public Class<?> getType() {
        return type;
    }

    public Type getGenericType() {
        return type;
    }

    @Override
    public boolean equals(Object obj) {
        if (!(obj instanceof Field)) {
            return false;
        }
        Field other = (Field) obj;
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

    public String toGenericString() {
        return toString();
    }

    public native Object get(Object obj) throws IllegalArgumentException, IllegalAccessException;

    public native boolean getBoolean(Object obj) throws IllegalArgumentException, IllegalAccessException;

    public native byte getByte(Object obj) throws IllegalArgumentException, IllegalAccessException;

    public native char getChar(Object obj) throws IllegalArgumentException, IllegalAccessException;

    public native short getShort(Object obj) throws IllegalArgumentException, IllegalAccessException;

    public native int getInt(Object obj) throws IllegalArgumentException, IllegalAccessException;

    public native long getLong(Object obj) throws IllegalArgumentException, IllegalAccessException;

    public native float getFloat(Object obj) throws IllegalArgumentException, IllegalAccessException;

    public native double getDouble(Object obj) throws IllegalArgumentException, IllegalAccessException;

    public native void set(Object obj, Object value) throws IllegalArgumentException, IllegalAccessException;

    public native void setBoolean(Object obj, boolean z) throws IllegalArgumentException, IllegalAccessException;

    public native void setByte(Object obj, byte b) throws IllegalArgumentException, IllegalAccessException;

    public native void setChar(Object obj, char c) throws IllegalArgumentException, IllegalAccessException;

    public native void setShort(Object obj, short s) throws IllegalArgumentException, IllegalAccessException;

    public native void setInt(Object obj, int i) throws IllegalArgumentException, IllegalAccessException;

    public native void setLong(Object obj, long l) throws IllegalArgumentException, IllegalAccessException;

    public native void setFloat(Object obj, float f) throws IllegalArgumentException, IllegalAccessException;

    public native void setDouble(Object obj, double d) throws IllegalArgumentException, IllegalAccessException;

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
    public <T extends Annotation> T[] getAnnotationsByType(Class<T> annotationClass) {
        return getDeclaredAnnotationsByType(annotationClass);
    }

    @Override
    public native Annotation[] getDeclaredAnnotations();

    public AnnotatedType getAnnotatedType() {
        return null;
    }
}
