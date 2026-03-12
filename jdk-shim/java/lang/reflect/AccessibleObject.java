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
