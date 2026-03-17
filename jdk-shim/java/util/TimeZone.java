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

import java.io.Serializable;

public class TimeZone implements Serializable, Cloneable {
    public static final int SHORT = 0;
    public static final int LONG = 1;
    private String ID;

    public TimeZone() {
        this("UTC");
    }

    protected TimeZone(String id) {
        this.ID = (id == null) ? "UTC" : id;
    }

    public static TimeZone getTimeZone(String ID) {
        return new TimeZone(ID);
    }

    public static TimeZone getDefault() {
        return new TimeZone("UTC");
    }

    public String getID() {
        return ID;
    }

    public void setID(String ID) {
        this.ID = (ID == null) ? "UTC" : ID;
    }

    public String getDisplayName(boolean daylight, int style, Locale locale) {
        return ID;
    }

    public String getDisplayName() {
        return ID;
    }

    public int getRawOffset() {
        return 0;
    }

    public int getOffset(long date) {
        return 0;
    }

    public boolean useDaylightTime() {
        return false;
    }

    public boolean inDaylightTime(Date date) {
        return false;
    }

    @Override
    public Object clone() {
        return new TimeZone(ID);
    }
}
