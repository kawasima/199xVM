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

package java.net;

import java.io.Closeable;
import java.io.IOException;
import java.security.SecureClassLoader;

public class URLClassLoader extends SecureClassLoader implements Closeable {
    private URL[] urls;

    public URLClassLoader(URL[] urls, ClassLoader parent) {
        super(parent);
        this.urls = urls != null ? urls.clone() : new URL[0];
    }

    public URLClassLoader(URL[] urls) {
        super();
        this.urls = urls != null ? urls.clone() : new URL[0];
    }

    protected void addURL(URL url) {
        if (url == null) {
            return;
        }
        URL[] newUrls = new URL[urls.length + 1];
        System.arraycopy(urls, 0, newUrls, 0, urls.length);
        newUrls[urls.length] = url;
        urls = newUrls;
    }

    public URL[] getURLs() {
        return urls.clone();
    }

    @Override
    public void close() throws IOException {}
}
