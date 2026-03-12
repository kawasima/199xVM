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
