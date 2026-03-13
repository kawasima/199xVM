/*
 * Copyright (c) 1998, 2025, Oracle and/or its affiliates. All rights reserved.
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

package java.net;

import java.io.UnsupportedEncodingException;

/**
 * Utility class for HTML form decoding. This class contains static methods
 * for decoding a String from the {@code application/x-www-form-urlencoded}
 * MIME format.
 *
 * <p>This shim implementation supports UTF-8 encoding only.
 *
 * @author  Mark Chamness
 * @author  Michael McCloskey
 * @since   1.2
 */
public class URLDecoder {
    private URLDecoder() {}

    /**
     * Decodes a {@code application/x-www-form-urlencoded} string using UTF-8.
     *
     * @deprecated Use {@link #decode(String, String)} instead.
     * @param s the String to decode
     * @return the newly decoded String
     */
    @Deprecated
    public static String decode(String s) {
        return decodeInternal(s);
    }

    /**
     * Decodes a {@code application/x-www-form-urlencoded} string using the
     * specified encoding scheme.
     *
     * @param s the String to decode
     * @param enc The name of a supported character encoding.
     * @return the newly decoded String
     * @throws UnsupportedEncodingException If character encoding needs to be
     *         consulted, but named character encoding is not supported
     * @since 1.4
     */
    public static String decode(String s, String enc) throws UnsupportedEncodingException {
        if (enc == null) throw new NullPointerException("Charset");
        return decodeInternal(s);
    }

    private static String decodeInternal(String s) {
        int n = s.length();
        StringBuilder sb = new StringBuilder(n);
        byte[] bytes = new byte[n];
        int i = 0;
        while (i < n) {
            char c = s.charAt(i);
            if (c == '+') {
                sb.append(' ');
                i++;
            } else if (c == '%') {
                // Collect a run of percent-encoded bytes
                int numBytes = 0;
                while (i < n && s.charAt(i) == '%') {
                    if (i + 2 >= n)
                        break;
                    bytes[numBytes++] = (byte) Integer.parseInt(s.substring(i + 1, i + 3), 16);
                    i += 3;
                }
                sb.append(new String(bytes, 0, numBytes, "UTF-8"));
            } else {
                sb.append(c);
                i++;
            }
        }
        return sb.toString();
    }
}
