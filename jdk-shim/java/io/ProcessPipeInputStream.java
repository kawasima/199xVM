/*
 * Copyright (c) 1994, 2024, Oracle and/or its affiliates. All rights reserved.
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
 * Host-backed stdin pipe used by the launcher API.
 */
public final class ProcessPipeInputStream extends InputStream {
    private boolean closed;

    public ProcessPipeInputStream() {}

    private native int read0();

    private native int available0();

    private native void close0();

    private void ensureOpen() throws IOException {
        if (closed) {
            throw new IOException("Stream closed");
        }
    }

    @Override
    public int read() throws IOException {
        synchronized (this) {
            while (true) {
                ensureOpen();
                int value = read0();
                if (value != -2) {
                    return value;
                }
                try {
                    wait();
                } catch (InterruptedException e) {
                    throw new IOException("Interrupted while waiting for stdin");
                }
            }
        }
    }

    @Override
    public int available() throws IOException {
        synchronized (this) {
            ensureOpen();
            return available0();
        }
    }

    @Override
    public void close() throws IOException {
        synchronized (this) {
            if (closed) {
                return;
            }
            closed = true;
            close0();
            notifyAll();
        }
    }
}
