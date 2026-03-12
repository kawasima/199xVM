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
