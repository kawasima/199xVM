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

import java.util.Formatter;

/**
 * Minimal PrintStream stub.
 * println/print are handled natively by the VM.
 */
public class PrintStream {
    public native void println(String s);
    public native void println(Object o);
    public native void println(int i);
    public native void println();
    public native void print(String s);
    public native void print(Object o);
    public native void print(int i);

    public PrintStream format(String format, Object... args) {
        String s = new Formatter().format(format, args).toString();
        print(s);
        return this;
    }

    public PrintStream printf(String format, Object... args) {
        return format(format, args);
    }

    public PrintStream append(CharSequence csq) {
        print(csq == null ? "null" : csq.toString());
        return this;
    }

    public PrintStream append(CharSequence csq, int start, int end) {
        CharSequence seq = csq == null ? "null" : csq;
        print(seq.subSequence(start, end).toString());
        return this;
    }

    public PrintStream append(char c) {
        print(String.valueOf(c));
        return this;
    }
}
