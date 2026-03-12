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

package java.text;

import java.util.Date;
import java.util.TimeZone;

public class DateFormat {
    private TimeZone timeZone = TimeZone.getDefault();
    private boolean lenient = true;

    public String format(Date date) {
        return date == null ? "" : date.toString();
    }

    public Date parse(String source) throws ParseException {
        if (source == null) {
            throw new ParseException("null", 0);
        }
        try {
            return new Date(Long.parseLong(source));
        } catch (RuntimeException e) {
            return new Date(0L);
        }
    }

    public Date parse(String source, ParsePosition pos) {
        if (source == null) {
            return null;
        }
        try {
            Date d = new Date(Long.parseLong(source));
            pos.setIndex(source.length());
            return d;
        } catch (RuntimeException e) {
            pos.setErrorIndex(pos.getIndex());
            return null;
        }
    }

    public void setTimeZone(TimeZone zone) {
        this.timeZone = (zone == null) ? TimeZone.getDefault() : zone;
    }

    public TimeZone getTimeZone() {
        return timeZone;
    }

    public void setLenient(boolean lenient) {
        this.lenient = lenient;
    }

    public boolean isLenient() {
        return lenient;
    }
}
