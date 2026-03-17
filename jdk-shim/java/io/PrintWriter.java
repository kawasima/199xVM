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

public class PrintWriter extends Writer {
    private final Writer out;
    private final boolean autoFlush;

    public PrintWriter(Writer out) {
        this(out, false);
    }

    public PrintWriter(Writer out, boolean autoFlush) {
        super(out);
        if (out == null) throw new NullPointerException();
        this.out = out;
        this.autoFlush = autoFlush;
    }

    public void write(char[] cbuf, int off, int len) throws IOException {
        out.write(cbuf, off, len);
    }

    public void write(int c) throws IOException {
        out.write(c);
    }

    public void write(String s, int off, int len) throws IOException {
        out.write(s, off, len);
    }

    public void print(String s) throws IOException {
        out.write(s == null ? "null" : s);
    }

    public void print(Object obj) throws IOException {
        print(String.valueOf(obj));
    }

    public void println() throws IOException {
        out.write(System.lineSeparator());
        if (autoFlush) out.flush();
    }

    public void println(String s) throws IOException {
        print(s);
        println();
    }

    public void println(Object obj) throws IOException {
        print(obj);
        println();
    }

    public void flush() throws IOException {
        out.flush();
    }

    public void close() throws IOException {
        out.close();
    }
}
