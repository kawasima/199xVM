/*
 * Copyright (c) 1997, 2025, Oracle and/or its affiliates. All rights reserved.
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

package java.security;

public class SecureClassLoader extends ClassLoader {

    protected SecureClassLoader(ClassLoader parent) {
        super(parent);
    }

    protected SecureClassLoader() {
        super();
    }

    protected SecureClassLoader(String name, ClassLoader parent) {
        super(name, parent);
    }

    protected final Class<?> defineClass(String name, byte[] b, int off, int len,
            CodeSource cs) {
        return defineClass(name, b, off, len, (ProtectionDomain) null);
    }

    protected final Class<?> defineClass(String name, java.nio.ByteBuffer b,
            CodeSource cs) {
        // ByteBuffer variant — extract bytes and delegate
        byte[] bytes = new byte[b.remaining()];
        b.get(bytes);
        return defineClass(name, bytes, 0, bytes.length, (ProtectionDomain) null);
    }
}
