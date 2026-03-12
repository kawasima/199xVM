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

package java.util;

public class Formatter {
    private final StringBuilder sb;

    public Formatter() {
        this.sb = new StringBuilder();
    }

    public Formatter format(String fmt, Object... args) {
        int argIndex = 0;
        int i = 0;
        while (i < fmt.length()) {
            char c = fmt.charAt(i);
            if (c == '%' && i + 1 < fmt.length()) {
                char spec = fmt.charAt(i + 1);
                if (spec == 's' || spec == 'd' || spec == 'f') {
                    if (argIndex < args.length) {
                        Object arg = args[argIndex++];
                        sb.append(arg == null ? "null" : arg.toString());
                    }
                    i += 2;
                } else if (spec == '%') {
                    sb.append('%');
                    i += 2;
                } else if (spec == 'n') {
                    sb.append('\n');
                    i += 2;
                } else {
                    sb.append(c);
                    i++;
                }
            } else {
                sb.append(c);
                i++;
            }
        }
        return this;
    }

    @Override
    public String toString() {
        return sb.toString();
    }
}
