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

package java.time.zone;

public class ZoneRules {
    private static final ZoneRules UTC = new ZoneRules();

    public static ZoneRules of() { return UTC; }

    public static ZoneRules of(java.time.ZoneOffset offset) { return UTC; }

    public boolean isDaylightSavings(java.time.Instant instant) { return false; }

    public ZoneOffsetTransition getTransition(java.time.LocalDateTime ldt) { return null; }

    public java.time.ZoneOffset getOffset(java.time.Instant instant) { return java.time.ZoneOffset.UTC; }

    public java.time.ZoneOffset getOffset(java.time.LocalDateTime localDateTime) { return java.time.ZoneOffset.UTC; }

    public java.util.List<java.time.ZoneOffset> getValidOffsets(java.time.LocalDateTime localDateTime) {
        return java.util.List.of(java.time.ZoneOffset.UTC);
    }

    public boolean isValidOffset(java.time.LocalDateTime localDateTime, java.time.ZoneOffset offset) { return true; }

    public boolean isFixedOffset() { return true; }
}
