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

import java.util.Locale;

public interface TemporalField {
    TemporalUnit getBaseUnit();
    TemporalUnit getRangeUnit();
    ValueRange range();
    boolean isDateBased();
    boolean isTimeBased();

    default ValueRange rangeRefinedBy(TemporalAccessor temporal) { return range(); }
    default long getFrom(TemporalAccessor temporal) { return temporal.getLong(this); }
    default <R extends Temporal> R adjustInto(R temporal, long newValue) { return temporal; }
    default String getDisplayName(Locale locale) { return toString(); }
    default boolean isSupportedBy(TemporalAccessor temporal) { return temporal != null && temporal.isSupported(this); }
}
