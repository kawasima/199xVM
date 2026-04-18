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

package java.io;

public class ObjectStreamField implements Comparable<Object> {
    private final String name;
    private final String signature;
    private final Class<?> type;
    private final boolean unshared;
    private int offset;

    public ObjectStreamField(String name, Class<?> type) {
        this(name, type, false);
    }

    public ObjectStreamField(String name, Class<?> type, boolean unshared) {
        if (name == null) {
            throw new NullPointerException();
        }
        this.name = name;
        this.type = type;
        this.unshared = unshared;
        this.signature = signatureFor(type);
    }

    public String getName() {
        return name;
    }

    public Class<?> getType() {
        return type;
    }

    public char getTypeCode() {
        return signature.charAt(0);
    }

    public String getTypeString() {
        return isPrimitive() ? null : signature;
    }

    public int getOffset() {
        return offset;
    }

    protected void setOffset(int offset) {
        this.offset = offset;
    }

    public boolean isPrimitive() {
        char code = getTypeCode();
        return code != '[' && code != 'L';
    }

    public boolean isUnshared() {
        return unshared;
    }

    public int compareTo(Object other) {
        ObjectStreamField rhs = (ObjectStreamField) other;
        boolean leftPrimitive = isPrimitive();
        boolean rightPrimitive = rhs.isPrimitive();
        if (leftPrimitive != rightPrimitive) {
            return leftPrimitive ? -1 : 1;
        }
        return name.compareTo(rhs.name);
    }

    public String toString() {
        return signature + " " + name;
    }

    private static String signatureFor(Class<?> type) {
        if (type == null) {
            throw new NullPointerException();
        }
        if (type.isPrimitive()) {
            return primitiveSignature(type.getName());
        }
        String name = type.getName();
        if (name.startsWith("[")) {
            return name.replace('.', '/');
        }
        return "L" + name.replace('.', '/') + ";";
    }

    private static String primitiveSignature(String name) {
        if ("boolean".equals(name)) {
            return "Z";
        }
        if ("byte".equals(name)) {
            return "B";
        }
        if ("char".equals(name)) {
            return "C";
        }
        if ("short".equals(name)) {
            return "S";
        }
        if ("int".equals(name)) {
            return "I";
        }
        if ("long".equals(name)) {
            return "J";
        }
        if ("float".equals(name)) {
            return "F";
        }
        if ("double".equals(name)) {
            return "D";
        }
        if ("void".equals(name)) {
            return "V";
        }
        throw new IllegalArgumentException("not primitive: " + name);
    }
}
