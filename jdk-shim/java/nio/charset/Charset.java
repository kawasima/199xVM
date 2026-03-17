/*
 * Copyright (c) 2000, 2025, Oracle and/or its affiliates. All rights reserved.
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

package java.nio.charset;

import java.util.HashSet;
import java.util.Set;

public class Charset implements Comparable<Charset> {
    private static final Charset UTF_8 = new Charset("UTF-8", new String[] { "UTF8", "utf8" });

    private final String name;
    private final String[] aliases;

    protected Charset(String canonicalName, String[] aliases) {
        checkName(canonicalName);
        this.name = canonicalName;
        this.aliases = aliases == null ? new String[0] : aliases.clone();
    }

    public static boolean isSupported(String charsetName) {
        try {
            forName(charsetName);
            return true;
        } catch (IllegalCharsetNameException e) {
            return false;
        } catch (UnsupportedCharsetException e) {
            return false;
        }
    }

    public static Charset forName(String charsetName) {
        String normalized = normalize(charsetName);
        if ("UTF-8".equals(normalized) || "UTF8".equals(normalized)) {
            return UTF_8;
        }
        throw new UnsupportedCharsetException(charsetName);
    }

    public static Charset defaultCharset() {
        return UTF_8;
    }

    public final String name() {
        return name;
    }

    public final Set<String> aliases() {
        HashSet<String> copy = new HashSet<>();
        for (String alias : aliases) {
            copy.add(alias);
        }
        return copy;
    }

    public String displayName() {
        return name;
    }

    public final boolean isRegistered() {
        return !name.startsWith("X-") && !name.startsWith("x-");
    }

    public boolean contains(Charset cs) {
        return cs != null && name.equalsIgnoreCase(cs.name());
    }

    public final int compareTo(Charset that) {
        return this.name.compareToIgnoreCase(that.name);
    }

    public final int hashCode() {
        return name.toUpperCase().hashCode();
    }

    public final boolean equals(Object ob) {
        if (!(ob instanceof Charset other)) return false;
        return name.equalsIgnoreCase(other.name);
    }

    public final String toString() {
        return name;
    }

    private static String normalize(String charsetName) {
        if (charsetName == null) throw new IllegalArgumentException("Null charset name");
        checkName(charsetName);
        return charsetName.toUpperCase();
    }

    private static void checkName(String charsetName) {
        if (charsetName == null || charsetName.length() == 0) {
            throw new IllegalCharsetNameException(charsetName);
        }
        for (int i = 0; i < charsetName.length(); i++) {
            char c = charsetName.charAt(i);
            boolean ok =
                (c >= 'A' && c <= 'Z') ||
                (c >= 'a' && c <= 'z') ||
                (c >= '0' && c <= '9') ||
                c == '-' || c == '+' || c == '.' || c == ':' || c == '_';
            if (!ok || (i == 0 && charsetName.length() == 0)) {
                throw new IllegalCharsetNameException(charsetName);
            }
        }
    }
}
