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

package java.util.regex;

public final class Pattern {
    public static final int UNIX_LINES = 0x01;
    public static final int CASE_INSENSITIVE = 0x02;
    public static final int COMMENTS = 0x04;
    public static final int MULTILINE = 0x08;
    public static final int LITERAL = 0x10;
    public static final int DOTALL = 0x20;
    public static final int UNICODE_CASE = 0x40;
    public static final int CANON_EQ = 0x80;
    public static final int UNICODE_CHARACTER_CLASS = 0x100;

    private final String regex;
    private final int flags;

    private Pattern(String regex, int flags) {
        this.regex = (regex == null) ? "" : regex;
        this.flags = flags;
    }

    public static Pattern compile(String regex) {
        return new Pattern(regex, 0);
    }

    public static Pattern compile(String regex, int flags) {
        return new Pattern(regex, flags);
    }

    public static boolean matches(String regex, CharSequence input) {
        return compile(regex).matcher(input).matches();
    }

    public Matcher matcher(CharSequence input) {
        return new Matcher(this, input == null ? "" : input.toString());
    }

    public String pattern() {
        return regex;
    }

    public int flags() {
        return flags;
    }

    public String[] split(CharSequence input) {
        return split(input, 0);
    }

    public String[] split(CharSequence input, int limit) {
        String s = input == null ? "" : input.toString();
        if (regex.length() == 0) {
            return new String[] { s };
        }
        java.util.ArrayList<String> out = new java.util.ArrayList<>();
        int from = 0;
        int idx;
        while (from <= s.length() && (idx = s.substring(from).indexOf(regex)) >= 0) {
            idx += from;
            if (limit > 0 && out.size() + 1 >= limit) {
                break;
            }
            out.add(s.substring(from, idx));
            from = idx + regex.length();
        }
        out.add(s.substring(from));
        return out.toArray(new String[out.size()]);
    }

    @Override
    public String toString() {
        return regex;
    }
}
