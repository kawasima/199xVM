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

package java.time;

public abstract class ZoneId {
    protected ZoneId() {}

    public static ZoneId of(String zoneId) { return java.time.ZoneOffset.of(zoneId.equals("Z") || zoneId.startsWith("+") || zoneId.startsWith("-") ? zoneId : "+00:00"); }
    public static ZoneId ofOffset(String prefix, java.time.ZoneOffset offset) { return offset; }

    public abstract String getId();
    public abstract java.time.zone.ZoneRules getRules();
    public ZoneId normalized() { return this; }
    public java.time.ZoneOffset getOffset(long epochSecond) { return java.time.ZoneOffset.UTC; }

    public static ZoneId from(java.time.temporal.TemporalAccessor temporal) { throw new UnsupportedOperationException("stub"); }
    public static ZoneId systemDefault() { return java.time.ZoneOffset.UTC; }

    @Override
    public String toString() { return getId(); }
}
