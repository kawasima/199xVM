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

package java.util.concurrent;

public enum TimeUnit {
    NANOSECONDS,
    MICROSECONDS,
    MILLISECONDS,
    SECONDS,
    MINUTES,
    HOURS,
    DAYS;

    public long convert(long sourceDuration, TimeUnit sourceUnit) {
        return sourceUnit.toNanos(sourceDuration) / toNanos(1L);
    }

    public long toNanos(long duration) {
        return switch (this) {
            case NANOSECONDS -> duration;
            case MICROSECONDS -> duration * 1_000L;
            case MILLISECONDS -> duration * 1_000_000L;
            case SECONDS -> duration * 1_000_000_000L;
            case MINUTES -> duration * 60_000_000_000L;
            case HOURS -> duration * 3_600_000_000_000L;
            case DAYS -> duration * 86_400_000_000_000L;
        };
    }

    public long toMicros(long duration) {
        return toNanos(duration) / 1_000L;
    }

    public long toMillis(long duration) {
        return toNanos(duration) / 1_000_000L;
    }

    public long toSeconds(long duration) {
        return toNanos(duration) / 1_000_000_000L;
    }

    public long toMinutes(long duration) {
        return toSeconds(duration) / 60L;
    }

    public long toHours(long duration) {
        return toMinutes(duration) / 60L;
    }

    public long toDays(long duration) {
        return toHours(duration) / 24L;
    }

    public void sleep(long timeout) throws InterruptedException {
        Thread.sleep(toMillis(timeout));
    }

    public void timedJoin(Thread thread, long timeout) throws InterruptedException {
        thread.join(toMillis(timeout));
    }

    public void timedWait(Object obj, long timeout) throws InterruptedException {
        obj.wait(toMillis(timeout));
    }
}
