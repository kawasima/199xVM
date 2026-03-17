/*
 * Copyright (c) 1996, 2024, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */

package java.lang;

import java.lang.reflect.Type;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.RecordComponent;
import java.lang.annotation.Annotation;
import java.util.ArrayList;

public final class Class<T> implements Type {
    // Managed natively by the VM.
    // The VM creates Class objects and sets the name field.
    private String name;

    // Private constructor — only the VM creates Class instances.
    private Class() {}

    public native String getName();

    public String getSimpleName() {
        String n = getName();
        int lastDot = n.lastIndexOf((int) '.');
        return (lastDot >= 0) ? n.substring(lastDot + 1) : n;
    }

    public native boolean isInstance(Object obj);

    public native boolean isAssignableFrom(Class<?> cls);

    public native boolean isInterface();

    public native int getModifiers();

    public boolean isArray() {
        return getName().startsWith("[");
    }

    public boolean isPrimitive() {
        String n = getName();
        return "boolean".equals(n) || "byte".equals(n) || "char".equals(n)
            || "short".equals(n) || "int".equals(n) || "long".equals(n)
            || "float".equals(n) || "double".equals(n) || "void".equals(n);
    }

    @SuppressWarnings("unchecked")
    public T cast(Object obj) {
        if (obj != null && !isInstance(obj)) {
            throw new ClassCastException();
        }
        return (T) obj;
    }

    public native Class<?> getComponentType();

    public native Class<? super T> getSuperclass();

    public native Class<?>[] getInterfaces();

    public native Object[] getEnumConstants();

    public Class<?> getEnclosingClass() {
        return null;
    }

    public Class<?> getDeclaringClass() {
        return null;
    }

    public native boolean isRecord();

    public native RecordComponent[] getRecordComponents();

    public boolean isEnum() {
        return (getModifiers() & 0x4000) != 0;
    }

    public boolean isAnnotation() {
        return (getModifiers() & 0x2000) != 0;
    }

    public boolean isSynthetic() {
        return (getModifiers() & 0x1000) != 0;
    }

    public ClassLoader getClassLoader() {
        return null;
    }

    public <A extends Annotation> A getAnnotation(Class<A> annotationClass) {
        Annotation[] annotations = getDeclaredAnnotations();
        for (int i = 0; i < annotations.length; i++) {
            Annotation a = annotations[i];
            if (annotationClass.isInstance(a)) {
                return annotationClass.cast(a);
            }
        }
        return null;
    }

    public boolean isAnnotationPresent(Class<? extends Annotation> annotationClass) {
        return getAnnotation(annotationClass) != null;
    }

    public Annotation[] getAnnotations() {
        return getDeclaredAnnotations();
    }

    public native Annotation[] getDeclaredAnnotations();

    @SuppressWarnings("unchecked")
    public <A extends Annotation> A[] getAnnotationsByType(Class<A> annotationClass) {
        return (A[]) new Annotation[0];
    }

    @SuppressWarnings("unchecked")
    public <A extends Annotation> A[] getDeclaredAnnotationsByType(Class<A> annotationClass) {
        return (A[]) new Annotation[0];
    }

    private static native Class<?> forName0(String className);
    private static native Class<?> forName1(String className, boolean initialize, ClassLoader loader);

    public static Class<?> forName(String className) throws ClassNotFoundException {
        return forName1(className, true, ClassLoader.getSystemClassLoader());
    }

    public static Class<?> forName(String className, boolean initialize, ClassLoader loader)
            throws ClassNotFoundException {
        Class<?> c = forName1(className, initialize, loader);
        if (c == null) {
            throw new ClassNotFoundException(className);
        }
        return c;
    }

    private native Field[] getDeclaredFields0(boolean publicOnly);

    public Field[] getDeclaredFields() {
        return getDeclaredFields0(false);
    }

    private native Method[] getDeclaredMethods0(boolean publicOnly);

    public Method[] getDeclaredMethods() {
        return getDeclaredMethods0(false);
    }

    @SuppressWarnings("unchecked")
    private native Constructor<T>[] getDeclaredConstructors0(boolean publicOnly);

    @SuppressWarnings("unchecked")
    public Constructor<T>[] getDeclaredConstructors() {
        return getDeclaredConstructors0(false);
    }

    public Field getDeclaredField(String name) throws NoSuchFieldException {
        Field[] fields = getDeclaredFields();
        for (int i = 0; i < fields.length; i++) {
            if (name.equals(fields[i].getName())) {
                return fields[i];
            }
        }
        throw new NoSuchFieldException(name);
    }

    public Method getDeclaredMethod(String name, Class<?>... parameterTypes) throws NoSuchMethodException {
        Method[] methods = getDeclaredMethods();
        for (int i = 0; i < methods.length; i++) {
            Method m = methods[i];
            if (!name.equals(m.getName())) {
                continue;
            }
            Class<?>[] params = m.getParameterTypes();
            if (params.length != parameterTypes.length) {
                continue;
            }
            boolean same = true;
            for (int j = 0; j < params.length; j++) {
                if (params[j] != parameterTypes[j]) {
                    same = false;
                    break;
                }
            }
            if (same) {
                return m;
            }
        }
        throw new NoSuchMethodException(name);
    }

    @SuppressWarnings("unchecked")
    public Constructor<T> getDeclaredConstructor(Class<?>... parameterTypes) throws NoSuchMethodException {
        Constructor<?>[] ctors = getDeclaredConstructors();
        for (int i = 0; i < ctors.length; i++) {
            Constructor<?> c = ctors[i];
            Class<?>[] params = c.getParameterTypes();
            if (params.length != parameterTypes.length) {
                continue;
            }
            boolean same = true;
            for (int j = 0; j < params.length; j++) {
                if (params[j] != parameterTypes[j]) {
                    same = false;
                    break;
                }
            }
            if (same) {
                return (Constructor<T>) c;
            }
        }
        throw new NoSuchMethodException(getName());
    }

    public Field[] getFields() {
        ArrayList<Field> out = new ArrayList<>();
        Class<?> c = this;
        while (c != null) {
            Field[] fields = c.getDeclaredFields0(true);
            for (int i = 0; i < fields.length; i++) {
                out.add(fields[i]);
            }
            c = c.getSuperclass();
        }
        Field[] arr = new Field[out.size()];
        for (int i = 0; i < out.size(); i++) {
            arr[i] = out.get(i);
        }
        return arr;
    }

    public Method[] getMethods() {
        ArrayList<Method> out = new ArrayList<>();
        Class<?> c = this;
        while (c != null) {
            Method[] methods = c.getDeclaredMethods0(true);
            for (int i = 0; i < methods.length; i++) {
                out.add(methods[i]);
            }
            c = c.getSuperclass();
        }
        Method[] arr = new Method[out.size()];
        for (int i = 0; i < out.size(); i++) {
            arr[i] = out.get(i);
        }
        return arr;
    }

    public Constructor<?>[] getConstructors() {
        return getDeclaredConstructors0(true);
    }

    public Field getField(String name) throws NoSuchFieldException {
        Class<?> c = this;
        while (c != null) {
            try {
                return c.getDeclaredField(name);
            } catch (NoSuchFieldException ignored) {
            }
            c = c.getSuperclass();
        }
        throw new NoSuchFieldException(name);
    }

    public Method getMethod(String name, Class<?>... parameterTypes) throws NoSuchMethodException {
        Class<?> c = this;
        while (c != null) {
            Method[] methods = c.getDeclaredMethods0(true);
            for (int i = 0; i < methods.length; i++) {
                Method m = methods[i];
                if (!name.equals(m.getName())) {
                    continue;
                }
                Class<?>[] params = m.getParameterTypes();
                if (params.length != parameterTypes.length) {
                    continue;
                }
                boolean same = true;
                for (int j = 0; j < params.length; j++) {
                    if (params[j] != parameterTypes[j]) {
                        same = false;
                        break;
                    }
                }
                if (same) {
                    return m;
                }
            }
            c = c.getSuperclass();
        }
        throw new NoSuchMethodException(name);
    }

    @SuppressWarnings("unchecked")
    public Constructor<T> getConstructor(Class<?>... parameterTypes) throws NoSuchMethodException {
        Constructor<?>[] ctors = getDeclaredConstructors0(true);
        for (int i = 0; i < ctors.length; i++) {
            Constructor<?> c = ctors[i];
            Class<?>[] params = c.getParameterTypes();
            if (params.length != parameterTypes.length) {
                continue;
            }
            boolean same = true;
            for (int j = 0; j < params.length; j++) {
                if (params[j] != parameterTypes[j]) {
                    same = false;
                    break;
                }
            }
            if (same) {
                return (Constructor<T>) c;
            }
        }
        throw new NoSuchMethodException(getName());
    }

    public String getTypeName() {
        return getName();
    }

    public boolean desiredAssertionStatus() {
        return false;
    }

    @Override
    public String toString() {
        return "class " + getName();
    }
}
