/*
 * Copyright (c) 1995, 2025, Oracle and/or its affiliates. All rights reserved.
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

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;

/**
 * The abstract class {@code URLConnection} is the superclass of all classes
 * that represent a communications link between the application and a URL.
 *
 * <p>This shim provides only the type; all operations throw
 * {@link UnsupportedOperationException} as network I/O is not supported
 * in 199xVM.
 *
 * @author  James Gosling
 * @since   1.0
 */
public abstract class URLConnection {

    protected URL url;

    protected URLConnection(URL url) {
        this.url = url;
    }

    public URL getURL() {
        return url;
    }

    public abstract void connect() throws IOException;

    public InputStream getInputStream() throws IOException {
        throw new UnsupportedOperationException("Network I/O not supported in 199xVM");
    }

    public OutputStream getOutputStream() throws IOException {
        throw new UnsupportedOperationException("Network I/O not supported in 199xVM");
    }

    public String getContentType() {
        return null;
    }

    public int getContentLength() {
        return -1;
    }

    public long getContentLengthLong() {
        return -1;
    }

    public long getLastModified() {
        return 0;
    }

    public String getHeaderField(String name) {
        return null;
    }

    public void setDoInput(boolean doinput) {}

    public void setDoOutput(boolean dooutput) {}

    public boolean getDoInput() { return true; }

    public boolean getDoOutput() { return false; }

    @Override
    public String toString() {
        return getClass().getName() + ":" + url;
    }
}
