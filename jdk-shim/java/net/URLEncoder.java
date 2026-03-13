/*
 * Copyright (c) 1995, 2025, Oracle and/or its affiliates. All rights reserved.
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
 * Utility class for HTML form encoding. This class contains static methods
 * for converting a String to the {@code application/x-www-form-urlencoded}
 * MIME format.
 *
 * <p>This shim implementation supports UTF-8 encoding only.
 *
 * @author  Herb Jellinek
 * @since   1.0
 */
public class URLEncoder {
    private URLEncoder() {}

    private static final String DONT_NEED_ENCODING = "abcdefghijklmnopqrstuvwxyz"
            + "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
            + "0123456789"
            + "-_.*";

    /**
     * Translates a string into {@code application/x-www-form-urlencoded}
     * format using UTF-8.
     *
     * @deprecated Use {@link #encode(String, String)} instead.
     * @param   s   String to be translated.
     * @return  the translated String.
     */
    @Deprecated
    public static String encode(String s) {
        return encodeInternal(s);
    }

    /**
     * Translates a string into {@code application/x-www-form-urlencoded}
     * format using the specified encoding scheme.
     *
     * @param   s   String to be translated.
     * @param   enc The name of a supported character encoding.
     * @return  the translated String.
     * @throws UnsupportedEncodingException If the named encoding is not supported
     * @since 1.4
     */
    public static String encode(String s, String enc) throws UnsupportedEncodingException {
        if (enc == null) throw new NullPointerException("Charset");
        // Only UTF-8 is supported; other charset names are accepted but mapped to UTF-8
        return encodeInternal(s);
    }

    private static String encodeInternal(String s) {
        StringBuilder sb = new StringBuilder();
        byte[] bytes = s.getBytes();
        for (byte rawByte : bytes) {
            int b = rawByte & 0xFF;
            if (DONT_NEED_ENCODING.indexOf(b) >= 0) {
                sb.append((char) b);
            } else if (b == ' ') {
                sb.append('+');
            } else {
                sb.append('%');
                char hex1 = Character.forDigit((b >> 4) & 0xF, 16);
                char hex2 = Character.forDigit(b & 0xF, 16);
                sb.append(Character.toUpperCase(hex1));
                sb.append(Character.toUpperCase(hex2));
            }
        }
        return sb.toString();
    }
}
