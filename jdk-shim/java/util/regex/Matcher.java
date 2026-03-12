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

public final class Matcher {
    private final Pattern pattern;
    private String input;
    private int searchIndex;
    private int matchStart = -1;
    private int matchEnd = -1;

    Matcher(Pattern pattern, String input) {
        this.pattern = pattern;
        this.input = (input == null) ? "" : input;
        this.searchIndex = 0;
    }

    public Pattern pattern() {
        return pattern;
    }

    public Matcher reset() {
        this.searchIndex = 0;
        this.matchStart = -1;
        this.matchEnd = -1;
        return this;
    }

    public Matcher reset(CharSequence input) {
        this.input = (input == null) ? "" : input.toString();
        return reset();
    }

    public boolean matches() {
        String r = pattern.pattern();
        if (".*".equals(r)) {
            matchStart = 0;
            matchEnd = input.length();
            return true;
        }
        if (r.equals(input)) {
            matchStart = 0;
            matchEnd = input.length();
            return true;
        }
        matchStart = -1;
        matchEnd = -1;
        return false;
    }

    public boolean find() {
        String r = pattern.pattern();
        if (r.length() == 0) {
            matchStart = -1;
            matchEnd = -1;
            return false;
        }
        int at = -1;
        if (searchIndex <= input.length()) {
            int rel = input.substring(searchIndex).indexOf(r);
            if (rel >= 0) {
                at = searchIndex + rel;
            }
        }
        if (at < 0) {
            matchStart = -1;
            matchEnd = -1;
            return false;
        }
        matchStart = at;
        matchEnd = at + r.length();
        searchIndex = matchEnd;
        return true;
    }

    public int start() {
        return matchStart;
    }

    public int end() {
        return matchEnd;
    }

    public String group() {
        if (matchStart < 0 || matchEnd < 0) {
            return null;
        }
        return input.substring(matchStart, matchEnd);
    }

    public String group(int group) {
        return group == 0 ? group() : null;
    }

    public String group(String name) {
        return group();
    }

    public String replaceAll(String replacement) {
        String r = pattern.pattern();
        if (r.length() == 0) {
            return input;
        }
        String rep = replacement == null ? "" : replacement;
        StringBuilder sb = new StringBuilder();
        int from = 0;
        int at;
        while (from <= input.length()) {
            int rel = input.substring(from).indexOf(r);
            if (rel < 0) {
                break;
            }
            at = from + rel;
            sb.append(input.substring(from, at));
            sb.append(rep);
            from = at + r.length();
        }
        sb.append(input.substring(from));
        return sb.toString();
    }
}
