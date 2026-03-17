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

import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.SortedMap;
import java.util.TreeMap;

/**
 * A named mapping between sequences of sixteen-bit Unicode code units and
 * sequences of bytes.
 *
 * <p>199xVM shim: hardcodes the standard charsets (UTF-8, US-ASCII,
 * ISO-8859-1, UTF-16, UTF-16BE, UTF-16LE). SPI provider lookup is removed.
 * Encode/decode methods throw {@code UnsupportedOperationException}.
 *
 * @author Mark Reinhold
 * @author JSR-51 Expert Group
 * @since 1.4
 */
public abstract class Charset implements Comparable<Charset> {

    private static final Map<String, Charset> CHARSETS = new HashMap<>();

    static {
        register(new SimpleCharset("UTF-8", new String[]{"utf8", "UTF8", "unicode-1-1-utf-8"}));
        register(new SimpleCharset("US-ASCII", new String[]{"ascii", "ASCII", "iso-ir-6", "ANSI_X3.4-1986", "cp367", "csASCII", "iso_646.irv:1991", "646", "ISO646-US", "us", "IBM367", "ANSI_X3.4-1968"}));
        register(new SimpleCharset("ISO-8859-1", new String[]{"iso-ir-100", "ISO_8859-1", "latin1", "l1", "IBM819", "cp819", "csISOLatin1", "819", "ISO8859_1", "ISO_8859-1:1987", "ISO_8859_1", "8859_1", "ISO8859-1"}));
        register(new SimpleCharset("UTF-16", new String[]{"utf16", "UTF_16", "unicode", "UnicodeBig"}));
        register(new SimpleCharset("UTF-16BE", new String[]{"X-UTF-16BE", "UTF_16BE", "ISO-10646-UCS-2", "UnicodeBigUnmarked"}));
        register(new SimpleCharset("UTF-16LE", new String[]{"X-UTF-16LE", "UTF_16LE", "UnicodeLittleUnmarked"}));
    }

    private static void register(SimpleCharset cs) {
        CHARSETS.put(cs.name().toUpperCase(Locale.ROOT), cs);
        for (String alias : cs.aliases()) {
            CHARSETS.put(alias.toUpperCase(Locale.ROOT), cs);
        }
    }

    private final String name;
    private final Set<String> aliases;

    protected Charset(String canonicalName, String[] aliases) {
        this.name = canonicalName;
        Set<String> as = new HashSet<>();
        if (aliases != null) {
            for (String a : aliases) as.add(a);
        }
        this.aliases = Collections.unmodifiableSet(as);
    }

    public static Charset forName(String charsetName) {
        if (charsetName == null) throw new IllegalArgumentException("null");
        Charset cs = CHARSETS.get(charsetName.toUpperCase(Locale.ROOT));
        if (cs != null) return cs;
        throw new UnsupportedCharsetException(charsetName);
    }

    public static Charset forName(String charsetName, Charset fallback) {
        try {
            return forName(charsetName);
        } catch (UnsupportedCharsetException e) {
            return fallback;
        }
    }

    public static boolean isSupported(String charsetName) {
        return CHARSETS.containsKey(charsetName.toUpperCase(Locale.ROOT));
    }

    public static SortedMap<String, Charset> availableCharsets() {
        TreeMap<String, Charset> map = new TreeMap<>(String.CASE_INSENSITIVE_ORDER);
        for (Charset cs : CHARSETS.values()) {
            map.put(cs.name(), cs);
        }
        return Collections.unmodifiableSortedMap(map);
    }

    public static Charset defaultCharset() {
        return forName("UTF-8");
    }

    public final String name() {
        return name;
    }

    public final Set<String> aliases() {
        return aliases;
    }

    public String displayName() {
        return name;
    }

    public String displayName(Locale locale) {
        return name;
    }

    public final boolean isRegistered() {
        return !name.startsWith("X-") && !name.startsWith("x-");
    }

    public boolean canEncode() {
        return true;
    }

    public abstract boolean contains(Charset cs);

    public final String toString() {
        return name();
    }

    public final boolean equals(Object ob) {
        if (!(ob instanceof Charset)) return false;
        return name.equals(((Charset) ob).name());
    }

    public final int hashCode() {
        return name.hashCode();
    }

    public final int compareTo(Charset that) {
        return name().compareToIgnoreCase(that.name());
    }

    private static final class SimpleCharset extends Charset {
        SimpleCharset(String name, String[] aliases) {
            super(name, aliases);
        }

        @Override
        public boolean contains(Charset cs) {
            return this.equals(cs);
        }
    }
}
