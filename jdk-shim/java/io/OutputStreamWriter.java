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

import java.nio.charset.Charset;

/**
 * An OutputStreamWriter is a bridge from character streams to byte streams.
 *
 * <p>199xVM shim: simplified — writes each char as a single byte (Latin-1).
 * Full UTF-8 encoding would require CharsetEncoder.
 *
 * @author      Mark Reinhold
 * @since       1.1
 */
public class OutputStreamWriter extends Writer {

    private final OutputStream out;
    private final String charsetName;
    private boolean closed = false;

    public OutputStreamWriter(OutputStream out, String charsetName)
            throws UnsupportedEncodingException {
        super(out);
        if (charsetName == null) throw new NullPointerException("charsetName");
        this.out = out;
        this.charsetName = charsetName;
    }

    public OutputStreamWriter(OutputStream out) {
        super(out);
        this.out = out;
        this.charsetName = Charset.defaultCharset().name();
    }

    public OutputStreamWriter(OutputStream out, Charset cs) {
        super(out);
        this.out = out;
        this.charsetName = cs.name();
    }

    public String getEncoding() {
        return closed ? null : charsetName;
    }

    void flushBuffer() throws IOException {
        if (closed) throw new IOException("Stream closed");
        out.flush();
    }

    public void write(int c) throws IOException {
        if (closed) throw new IOException("Stream closed");
        out.write(c & 0xFF);
    }

    public void write(char[] cbuf, int off, int len) throws IOException {
        if (closed) throw new IOException("Stream closed");
        for (int i = 0; i < len; i++) {
            out.write(cbuf[off + i] & 0xFF);
        }
    }

    public void write(String str, int off, int len) throws IOException {
        if (closed) throw new IOException("Stream closed");
        for (int i = 0; i < len; i++) {
            out.write(str.charAt(off + i) & 0xFF);
        }
    }

    public void flush() throws IOException {
        if (closed) throw new IOException("Stream closed");
        out.flush();
    }

    public void close() throws IOException {
        if (!closed) {
            closed = true;
            out.flush();
            out.close();
        }
    }
}
