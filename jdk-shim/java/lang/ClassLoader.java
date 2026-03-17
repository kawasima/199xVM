/*
 * Copyright (c) 2013, 2024, Oracle and/or its affiliates. All rights reserved.
 * Copyright (c) 2019, Azul Systems, Inc. All rights reserved.
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

import java.io.InputStream;
import java.io.IOException;
import java.net.URL;
import java.util.Enumeration;

public abstract class ClassLoader {

    private final ClassLoader parent;

    protected ClassLoader() {
        this(getSystemClassLoader());
    }

    protected ClassLoader(ClassLoader parent) {
        this.parent = parent;
    }

    protected ClassLoader(String name, ClassLoader parent) {
        this(parent);
    }

    public static native ClassLoader getSystemClassLoader();

    public static URL getSystemResource(String name) {
        return getSystemClassLoader().getResource(name);
    }

    public static InputStream getSystemResourceAsStream(String name) {
        return getSystemClassLoader().getResourceAsStream(name);
    }

    public static Enumeration<URL> getSystemResources(String name) throws IOException {
        return getSystemClassLoader().getResources(name);
    }

    public Class<?> loadClass(String name) throws ClassNotFoundException {
        return loadClass(name, false);
    }

    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
        Class<?> c = findLoadedClass(name);
        if (c == null) {
            try {
                if (parent != null) {
                    c = parent.loadClass(name, false);
                }
            } catch (ClassNotFoundException e) {
                // parent didn't find it
            }
            if (c == null) {
                c = findClass(name);
            }
        }
        if (resolve) {
            resolveClass(c);
        }
        return c;
    }

    protected native Class<?> findLoadedClass(String name);

    protected Class<?> findClass(String name) throws ClassNotFoundException {
        throw new ClassNotFoundException(name);
    }

    protected final native Class<?> defineClass(String name, byte[] b, int off, int len);

    protected final Class<?> defineClass(String name, byte[] b, int off, int len,
            java.security.ProtectionDomain protectionDomain) {
        return defineClass(name, b, off, len);
    }

    protected final void resolveClass(Class<?> c) {
        // intentional no-op in 199xVM
    }

    public final ClassLoader getParent() {
        return parent;
    }

    public native URL getResource(String name);

    public native InputStream getResourceAsStream(String name);

    public Enumeration<URL> getResources(String name) throws IOException {
        return java.util.Collections.emptyEnumeration();
    }

    protected URL findResource(String name) {
        return null;
    }

    protected Enumeration<URL> findResources(String name) throws IOException {
        return java.util.Collections.emptyEnumeration();
    }

    protected Package getPackage(String name) {
        return null;
    }

    protected Package[] getPackages() {
        return new Package[0];
    }
}
