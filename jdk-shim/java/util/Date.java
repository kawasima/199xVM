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

public class Date implements Serializable, Cloneable, Comparable<Date> {
    private long fastTime;

    public Date() {
        this(System.currentTimeMillis());
    }

    public Date(long date) {
        this.fastTime = date;
    }

    public long getTime() {
        return fastTime;
    }

    public void setTime(long time) {
        this.fastTime = time;
    }

    public boolean before(Date when) {
        return this.fastTime < when.fastTime;
    }

    public boolean after(Date when) {
        return this.fastTime > when.fastTime;
    }

    @Override
    public int compareTo(Date anotherDate) {
        long thisTime = this.fastTime;
        long anotherTime = anotherDate.fastTime;
        return (thisTime < anotherTime ? -1 : (thisTime == anotherTime ? 0 : 1));
    }

    @Override
    public Object clone() {
        return new Date(fastTime);
    }

    @Override
    public String toString() {
        return Long.toString(fastTime);
    }
}
