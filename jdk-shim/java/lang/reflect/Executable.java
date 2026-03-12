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

package java.lang.reflect;

import java.lang.annotation.Annotation;

public abstract class Executable extends AccessibleObject implements Member, GenericDeclaration {
    Executable() {}

    abstract byte[] getAnnotationBytes();

    abstract boolean hasGenericInformation();

    abstract String getGenericSignature();

    abstract AnnotatedType getAnnotatedReturnType0(Type returnType);

    abstract void specificToStringHeader(StringBuilder sb);

    abstract void specificToGenericStringHeader(StringBuilder sb);

    abstract Class<?>[] getSharedParameterTypes();

    abstract Class<?>[] getSharedExceptionTypes();

    @Override
    public abstract Class<?> getDeclaringClass();

    @Override
    public abstract String getName();

    @Override
    public abstract int getModifiers();

    @Override
    public abstract TypeVariable<?>[] getTypeParameters();

    public abstract Class<?>[] getParameterTypes();

    public int getParameterCount() {
        return getParameterTypes().length;
    }

    public Type[] getGenericParameterTypes() {
        Class<?>[] params = getParameterTypes();
        Type[] result = new Type[params.length];
        for (int i = 0; i < params.length; i++) {
            result[i] = params[i];
        }
        return result;
    }

    public Parameter[] getParameters() {
        return new Parameter[0];
    }

    public abstract Class<?>[] getExceptionTypes();

    public Type[] getGenericExceptionTypes() {
        Class<?>[] ex = getExceptionTypes();
        Type[] result = new Type[ex.length];
        for (int i = 0; i < ex.length; i++) {
            result[i] = ex[i];
        }
        return result;
    }

    public abstract String toGenericString();

    public boolean isVarArgs() {
        return (getModifiers() & Modifier.VARARGS) != 0;
    }

    @Override
    public boolean isSynthetic() {
        return Modifier.isSynthetic(getModifiers());
    }

    public native Annotation[][] getParameterAnnotations();

    public AnnotatedType getAnnotatedReturnType() {
        return getAnnotatedReturnType0(null);
    }

    public AnnotatedType getAnnotatedReceiverType() {
        return null;
    }

    public AnnotatedType[] getAnnotatedParameterTypes() {
        return new AnnotatedType[0];
    }

    public AnnotatedType[] getAnnotatedExceptionTypes() {
        return new AnnotatedType[0];
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
    public <T extends Annotation> T[] getAnnotationsByType(Class<T> annotationClass) {
        return getDeclaredAnnotationsByType(annotationClass);
    }

    @Override
    public Annotation[] getDeclaredAnnotations() {
        return new Annotation[0];
    }

    @Override
    public boolean isAnnotationPresent(Class<? extends Annotation> annotationClass) {
        return getAnnotation(annotationClass) != null;
    }
}
