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

/**
 * An InputStreamReader is a bridge from byte streams to character streams.
 *
 * <p>199xVM shim: simplified — delegates to underlying InputStream
 * treating each byte as a Latin-1 character. For UTF-8 multibyte,
 * full CharsetDecoder support would be needed.
 *
 * @author      Mark Reinhold
 * @since       1.1
 */
public class InputStreamReader extends Reader {

    private final InputStream in;
    private final String charsetName;
    private boolean closed = false;

    public InputStreamReader(InputStream in) {
        super(in);
        this.in = in;
        this.charsetName = Charset.defaultCharset().name();
    }

    public InputStreamReader(InputStream in, String charsetName)
            throws UnsupportedEncodingException {
        super(in);
        if (charsetName == null) throw new NullPointerException("charsetName");
        this.in = in;
        this.charsetName = charsetName;
    }

    public InputStreamReader(InputStream in, Charset cs) {
        super(in);
        this.in = in;
        this.charsetName = cs.name();
    }

    public String getEncoding() {
        return closed ? null : charsetName;
    }

    public int read() throws IOException {
        if (closed) throw new IOException("Stream closed");
        return in.read();
    }

    public int read(char[] cbuf, int off, int len) throws IOException {
        if (closed) throw new IOException("Stream closed");
        if (len == 0) return 0;
        int count = 0;
        for (int i = 0; i < len; i++) {
            int b = in.read();
            if (b == -1) return count == 0 ? -1 : count;
            cbuf[off + i] = (char) b;
            count++;
        }
        return count;
    }

    public boolean ready() throws IOException {
        if (closed) throw new IOException("Stream closed");
        return in.available() > 0;
    }

    public void close() throws IOException {
        if (!closed) {
            closed = true;
            in.close();
        }
    }
}
