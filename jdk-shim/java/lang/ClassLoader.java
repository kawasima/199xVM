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

import java.io.IOException;
import java.io.InputStream;
import java.net.URL;
import java.net.MalformedURLException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Enumeration;

public class ClassLoader {
    private final ClassLoader parent;

    protected ClassLoader() {
        this(null);
    }

    protected ClassLoader(ClassLoader parent) {
        this.parent = parent;
    }

    protected ClassLoader(String name, ClassLoader parent) {
        this(parent);
    }

    public static native ClassLoader getSystemClassLoader();

    private static native int resourceCount0(String name);

    private static String normalizeResourceName(String name) {
        if (name == null) {
            throw new NullPointerException("name");
        }
        int index = 0;
        while (index < name.length() && name.charAt(index) == '/') {
            index++;
        }
        return index == 0 ? name : name.substring(index);
    }

    private static URL newBundleUrl(String name, int index) {
        try {
            String suffix = index == 0 ? "" : "?entry=" + index;
            return new URL("bundle", "", "/" + name + suffix);
        } catch (MalformedURLException e) {
            return null;
        }
    }

    public static URL getSystemResource(String name) {
        String resourceName = normalizeResourceName(name);
        return resourceCount0(resourceName) > 0 ? newBundleUrl(resourceName, 0) : null;
    }

    public static InputStream getSystemResourceAsStream(String name) {
        URL url = getSystemResource(name);
        if (url == null) {
            return null;
        }
        try {
            return url.openStream();
        } catch (IOException e) {
            return null;
        }
    }

    public static Enumeration<URL> getSystemResources(String name) throws IOException {
        String resourceName = normalizeResourceName(name);
        int count = resourceCount0(resourceName);
        if (count == 0) {
            return Collections.emptyEnumeration();
        }
        ArrayList<URL> urls = new ArrayList<>(count);
        for (int i = 0; i < count; i++) {
            URL url = newBundleUrl(resourceName, i);
            if (url != null) {
                urls.add(url);
            }
        }
        return Collections.enumeration(urls);
    }

    public Class<?> loadClass(String name) throws ClassNotFoundException {
        return loadClass(name, false);
    }

    public URL getResource(String name) {
        return getSystemResource(name);
    }

    public InputStream getResourceAsStream(String name) {
        return getSystemResourceAsStream(name);
    }

    public Enumeration<URL> getResources(String name) throws IOException {
        return getSystemResources(name);
    }

    protected URL findResource(String name) {
        return getSystemResource(name);
    }

    protected Enumeration<URL> findResources(String name) throws IOException {
        return getSystemResources(name);
    }

    // Simplified parent-delegation: checks the local registry via native stubs only.
    // No parent-loader chain and no link-resolution step (the 'resolve' flag is ignored)
    // — both are intentional simplifications for 199xVM's pre-bundled class model.
    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
        Class<?> c = findLoadedClass(name);
        if (c == null) {
            c = findClass(name);
        }
        return c;
    }

    protected native Class<?> findLoadedClass(String name);

    protected native Class<?> findClass(String name) throws ClassNotFoundException;

    public final ClassLoader getParent() {
        return parent;
    }

    protected final void resolveClass(Class<?> c) {}

    protected final Class<?> defineClass(String name, byte[] b, int off, int len) {
        return null;
    }
}
