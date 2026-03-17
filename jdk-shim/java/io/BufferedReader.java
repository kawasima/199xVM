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

public class BufferedReader extends Reader {
    protected Reader in;

    public BufferedReader(Reader in) {
        this(in, 8192);
    }

    public BufferedReader(Reader in, int sz) {
        super(in);
        this.in = in == null ? Reader.nullReader() : in;
    }

    public int read(char[] cbuf, int off, int len) throws IOException {
        return in.read(cbuf, off, len);
    }

    public int read() throws IOException {
        return in.read();
    }

    public boolean ready() throws IOException {
        return in.ready();
    }

    public String readLine() throws IOException {
        StringBuilder sb = null;
        while (true) {
            int ch = in.read();
            if (ch == -1) {
                return sb == null ? null : sb.toString();
            }
            if (ch == '\n') {
                return sb == null ? "" : sb.toString();
            }
            if (ch == '\r') {
                in.mark(1);
                int next = in.read();
                if (next != '\n' && next != -1) {
                    in.reset();
                }
                return sb == null ? "" : sb.toString();
            }
            if (sb == null) {
                sb = new StringBuilder();
            }
            sb.append((char) ch);
        }
    }

    public void close() throws IOException {
        in.close();
    }

    public void mark(int readAheadLimit) throws IOException {
        in.mark(readAheadLimit);
    }

    public void reset() throws IOException {
        in.reset();
    }

    public boolean markSupported() {
        return in.markSupported();
    }
}
