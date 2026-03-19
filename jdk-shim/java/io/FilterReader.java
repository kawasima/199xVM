/*
 * Copyright (c) 1996, 2022, Oracle and/or its affiliates. All rights reserved.
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
 * Abstract class for reading filtered character streams.
 *
 * @author      Mark Reinhold
 * @since       1.1
 */

public abstract class FilterReader extends Reader {

    protected Reader in;

    protected FilterReader(Reader in) {
        super(in);
        this.in = in;
    }

    public int read() throws IOException {
        return in.read();
    }

    public int read(char[] cbuf, int off, int len) throws IOException {
        return in.read(cbuf, off, len);
    }

    public long skip(long n) throws IOException {
        return in.skip(n);
    }

    public boolean ready() throws IOException {
        return in.ready();
    }

    public boolean markSupported() {
        return in.markSupported();
    }

    public void mark(int readAheadLimit) throws IOException {
        in.mark(readAheadLimit);
    }

    public void reset() throws IOException {
        in.reset();
    }

    public void close() throws IOException {
        in.close();
    }
}
