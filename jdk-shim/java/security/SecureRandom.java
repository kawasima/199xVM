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

package java.security;

/**
 * 199xVM shim: SecureRandom backed by the VM's native CSPRNG
 * (getrandom crate → crypto.getRandomValues() on WASM).
 *
 * nextBytes() is native; all other randomness methods delegate through
 * the parent Random.next(int) which is overridden to pull from the
 * native source.
 */
public class SecureRandom extends java.util.Random {
    private static final long serialVersionUID = 4940670005562187L;

    public SecureRandom() {
        super(0L);
    }

    public SecureRandom(byte[] seed) {
        super(0L);
        // Seed is accepted but ignored — we always use CSPRNG.
    }

    @Override
    public void setSeed(long seed) {
        // Ignored — CSPRNG is self-seeding.
    }

    public void setSeed(byte[] seed) {
        // Ignored — CSPRNG is self-seeding.
    }

    /**
     * Fills the given byte array with cryptographically strong random bytes.
     * Delegated to the VM's native CSPRNG (getrandom / crypto.getRandomValues).
     */
    @Override
    public native void nextBytes(byte[] bytes);

    /**
     * Override next(int) to use CSPRNG instead of LCG.
     * This ensures nextInt(), nextLong(), nextDouble() etc. are all
     * cryptographically random.
     */
    @Override
    protected int next(int bits) {
        byte[] buf = new byte[4];
        nextBytes(buf);
        int val = ((buf[0] & 0xff) << 24) | ((buf[1] & 0xff) << 16)
                | ((buf[2] & 0xff) << 8)  | (buf[3] & 0xff);
        return val >>> (32 - bits);
    }

    public byte[] generateSeed(int numBytes) {
        byte[] seed = new byte[numBytes];
        nextBytes(seed);
        return seed;
    }

    public static SecureRandom getInstanceStrong() {
        return new SecureRandom();
    }
}
