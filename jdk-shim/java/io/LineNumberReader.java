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

public class LineNumberReader extends Reader {
    private final Reader in;
    private int lineNumber;

    public LineNumberReader(Reader in) {
        this(in, 8192);
    }

    public LineNumberReader(Reader in, int sz) {
        super(in);
        if (in == null) throw new NullPointerException();
        this.in = in;
        this.lineNumber = 0;
    }

    public void setLineNumber(int lineNumber) {
        this.lineNumber = lineNumber;
    }

    public int getLineNumber() {
        return lineNumber;
    }

    public int read() throws IOException {
        int ch = in.read();
        if (ch == '\n') {
            lineNumber++;
        }
        return ch;
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

    public String readLine() throws IOException {
        StringBuilder sb = new StringBuilder();
        for (;;) {
            int ch = read();
            if (ch < 0) {
                return sb.length() == 0 ? null : sb.toString();
            }
            if (ch == '\n') {
                return sb.toString();
            }
            if (ch == '\r') {
                int next = in.read();
                if (next == '\n') {
                    lineNumber++;
                } else if (next >= 0 && in instanceof PushbackReader pushback) {
                    pushback.unread(next);
                }
                return sb.toString();
            }
            sb.append((char) ch);
        }
    }

    public void close() throws IOException {
        in.close();
    }
}
