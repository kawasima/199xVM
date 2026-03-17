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

import java.nio.charset.Charset;

public class InputStreamReader extends Reader {
    private final InputStream in;
    private final Charset charset;

    public InputStreamReader(InputStream in) {
        this(in, Charset.defaultCharset());
    }

    public InputStreamReader(InputStream in, Charset cs) {
        super(in);
        if (in == null) throw new NullPointerException();
        if (cs == null) throw new NullPointerException();
        this.in = in;
        this.charset = cs;
    }

    public String getEncoding() {
        return charset.name();
    }

    public int read() throws IOException {
        int b = in.read();
        if (b < 0) return -1;
        return b & 0xff;
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

    public boolean ready() throws IOException {
        return false;
    }

    public void close() throws IOException {
        in.close();
    }
}
