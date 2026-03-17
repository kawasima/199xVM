/*
 * Copyright (c) 1996, 2025, Oracle and/or its affiliates. All rights reserved.
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

public class PushbackReader extends Reader {
    protected Reader in;
    private final char[] buf;
    private int pos;

    public PushbackReader(Reader in) {
        this(in, 1);
    }

    public PushbackReader(Reader in, int size) {
        super(in);
        if (in == null) throw new NullPointerException();
        if (size <= 0) throw new IllegalArgumentException("size <= 0");
        this.in = in;
        this.buf = new char[size];
        this.pos = size;
    }

    public int read() throws IOException {
        if (pos < buf.length) {
            return buf[pos++];
        }
        return in.read();
    }

    public int read(char[] cbuf, int off, int len) throws IOException {
        if (len == 0) return 0;
        int count = 0;
        while (count < len) {
            int ch = read();
            if (ch < 0) {
                return count == 0 ? -1 : count;
            }
            cbuf[off + count] = (char) ch;
            count++;
        }
        return count;
    }

    public void unread(int c) throws IOException {
        if (pos == 0) throw new IOException("Pushback buffer overflow");
        buf[--pos] = (char) c;
    }

    public boolean ready() throws IOException {
        return pos < buf.length || in.ready();
    }

    public void close() throws IOException {
        in.close();
    }
}
