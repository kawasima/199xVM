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

package java.time.temporal;

public final class TemporalQueries {
    private TemporalQueries() {}

    private static final TemporalQuery<java.time.ZoneId> ZONE_ID = temporal -> null;
    private static final TemporalQuery<java.time.ZoneId> ZONE = temporal -> null;
    private static final TemporalQuery<java.time.ZoneOffset> OFFSET = temporal -> null;
    private static final TemporalQuery<java.time.chrono.Chronology> CHRONOLOGY = temporal -> null;
    private static final TemporalQuery<java.time.LocalDate> LOCAL_DATE = temporal -> null;
    private static final TemporalQuery<java.time.LocalTime> LOCAL_TIME = temporal -> null;
    private static final TemporalQuery<java.time.temporal.ChronoUnit> PRECISION = temporal -> null;

    public static TemporalQuery<java.time.ZoneId> zoneId() { return ZONE_ID; }
    public static TemporalQuery<java.time.ZoneId> zone() { return ZONE; }
    public static TemporalQuery<java.time.ZoneOffset> offset() { return OFFSET; }
    public static TemporalQuery<java.time.chrono.Chronology> chronology() { return CHRONOLOGY; }
    public static TemporalQuery<java.time.LocalDate> localDate() { return LOCAL_DATE; }
    public static TemporalQuery<java.time.LocalTime> localTime() { return LOCAL_TIME; }
    public static TemporalQuery<java.time.temporal.ChronoUnit> precision() { return PRECISION; }
}
