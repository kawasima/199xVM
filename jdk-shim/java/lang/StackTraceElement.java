/*
 * Copyright (c) 2000, 2025, Oracle and/or its affiliates. All rights reserved.
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

import java.util.Objects;

/**
 * An element in a stack trace.
 *
 * <p>199xVM shim: module-related fields are omitted. The public API
 * matches JDK 25 for the non-module constructors and accessors.
 *
 * @since 1.4
 * @author Josh Bloch
 */
public final class StackTraceElement implements java.io.Serializable {

    private String declaringClass;
    private String methodName;
    private String fileName;
    private int lineNumber;

    public StackTraceElement(String declaringClass, String methodName,
            String fileName, int lineNumber) {
        this.declaringClass = Objects.requireNonNull(declaringClass, "Declaring class is null");
        this.methodName = Objects.requireNonNull(methodName, "Method name is null");
        this.fileName = fileName;
        this.lineNumber = lineNumber;
    }

    public StackTraceElement(String classLoaderName, String moduleName,
            String moduleVersion, String declaringClass,
            String methodName, String fileName, int lineNumber) {
        this(declaringClass, methodName, fileName, lineNumber);
    }

    public String getFileName() {
        return fileName;
    }

    public int getLineNumber() {
        return lineNumber;
    }

    public String getClassName() {
        return declaringClass;
    }

    public String getMethodName() {
        return methodName;
    }

    public boolean isNativeMethod() {
        return lineNumber == -2;
    }

    public String getModuleName() {
        return null;
    }

    public String getModuleVersion() {
        return null;
    }

    public String getClassLoaderName() {
        return null;
    }

    public String toString() {
        StringBuilder sb = new StringBuilder();
        sb.append(declaringClass).append('.').append(methodName);
        if (fileName != null) {
            sb.append('(').append(fileName);
            if (lineNumber >= 0) {
                sb.append(':').append(lineNumber);
            }
            sb.append(')');
        } else if (isNativeMethod()) {
            sb.append("(Native Method)");
        } else {
            sb.append("(Unknown Source)");
        }
        return sb.toString();
    }

    public boolean equals(Object obj) {
        if (obj == this) return true;
        if (!(obj instanceof StackTraceElement)) return false;
        StackTraceElement e = (StackTraceElement) obj;
        return Objects.equals(declaringClass, e.declaringClass)
            && Objects.equals(methodName, e.methodName)
            && Objects.equals(fileName, e.fileName)
            && lineNumber == e.lineNumber;
    }

    public int hashCode() {
        return Objects.hash(declaringClass, methodName, fileName, lineNumber);
    }
}
