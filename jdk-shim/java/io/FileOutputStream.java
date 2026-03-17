/*
 * Copyright (c) 1994, 2025, Oracle and/or its affiliates. All rights reserved.
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

/**
 * 199xVM stub: FileOutputStream is not supported (no filesystem).
 * Constructors throw UnsupportedOperationException.
 */
public class FileOutputStream extends OutputStream {

    public FileOutputStream(String name) throws FileNotFoundException {
        throw new FileNotFoundException("199xVM: no filesystem — " + name);
    }

    public FileOutputStream(String name, boolean append) throws FileNotFoundException {
        throw new FileNotFoundException("199xVM: no filesystem — " + name);
    }

    public FileOutputStream(File file) throws FileNotFoundException {
        throw new FileNotFoundException("199xVM: no filesystem — " + file.getPath());
    }

    public FileOutputStream(File file, boolean append) throws FileNotFoundException {
        throw new FileNotFoundException("199xVM: no filesystem — " + file.getPath());
    }

    public FileOutputStream(FileDescriptor fdObj) {
        // Allow construction with fd for stdout/stderr
    }

    public void write(int b) throws IOException {
        throw new IOException("199xVM: FileOutputStream not supported");
    }

    public void write(byte[] b, int off, int len) throws IOException {
        throw new IOException("199xVM: FileOutputStream not supported");
    }

    public void close() throws IOException {}
    public void flush() throws IOException {}
}
