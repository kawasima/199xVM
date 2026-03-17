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

package java.lang;

import java.io.Serializable;

public class Throwable implements Serializable {
    private String detailMessage;
    private Throwable cause = this;
    private StackTraceElement[] stackTrace = new StackTraceElement[0];

    public Throwable() {}
    public Throwable(String message) { this.detailMessage = message; }
    public Throwable(String message, Throwable cause) {
        this.detailMessage = message;
        this.cause = cause;
    }
    public Throwable(Throwable cause) {
        this.detailMessage = cause == null ? null : cause.toString();
        this.cause = cause;
    }
    protected Throwable(String message, Throwable cause, boolean enableSuppression, boolean writableStackTrace) {
        this.detailMessage = message;
        this.cause = cause;
    }

    public String getMessage() { return detailMessage; }
    public synchronized Throwable getCause() { return (cause == this ? null : cause); }
    public synchronized Throwable initCause(Throwable cause) {
        if (this.cause != this) throw new IllegalStateException("Can't overwrite cause");
        if (cause == this) throw new IllegalArgumentException("Self-causation not permitted");
        this.cause = cause;
        return this;
    }
    final void setCause(Throwable cause) {
        this.cause = cause;
    }

    public Throwable fillInStackTrace() {
        stackTrace = new StackTraceElement[0];
        return this;
    }

    public StackTraceElement[] getStackTrace() {
        return stackTrace.clone();
    }

    public void setStackTrace(StackTraceElement[] stackTrace) {
        if (stackTrace == null) {
            throw new NullPointerException();
        }
        this.stackTrace = stackTrace.clone();
    }

    public String toString() {
        String s = getClass().getName();
        String message = getMessage();
        return (message != null) ? (s + ": " + message) : s;
    }
}
