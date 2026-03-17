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

import java.io.PrintStream;
import java.io.PrintWriter;
import java.io.Serializable;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

public class Throwable implements Serializable {
    private String detailMessage;
    private Throwable cause = this;
    private StackTraceElement[] stackTrace = new StackTraceElement[0];
    private List<Throwable> suppressedExceptions = null;

    public Throwable() {
        fillInStackTrace();
    }
    public Throwable(String message) {
        this.detailMessage = message;
        fillInStackTrace();
    }
    public Throwable(String message, Throwable cause) {
        this.detailMessage = message;
        this.cause = cause;
        fillInStackTrace();
    }
    public Throwable(Throwable cause) {
        this.detailMessage = cause == null ? null : cause.toString();
        this.cause = cause;
        fillInStackTrace();
    }
    protected Throwable(String message, Throwable cause, boolean enableSuppression, boolean writableStackTrace) {
        this.detailMessage = message;
        this.cause = cause;
        if (writableStackTrace) {
            fillInStackTrace();
        }
    }

    public String getMessage() { return detailMessage; }
    public String getLocalizedMessage() { return getMessage(); }
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
    public String toString() {
        String s = getClass().getName();
        String message = getLocalizedMessage();
        return (message != null) ? (s + ": " + message) : s;
    }

    public StackTraceElement[] getStackTrace() {
        return stackTrace.clone();
    }

    public void setStackTrace(StackTraceElement[] stackTrace) {
        this.stackTrace = stackTrace.clone();
    }

    public Throwable fillInStackTrace() {
        // 199xVM does not yet capture real stack frames
        return this;
    }

    public void printStackTrace() {
        printStackTrace(System.err);
    }

    public void printStackTrace(PrintStream s) {
        s.println(this);
        for (StackTraceElement ste : stackTrace) {
            s.println("\tat " + ste);
        }
        Throwable ourCause = getCause();
        if (ourCause != null) {
            s.println("Caused by: " + ourCause);
            ourCause.printStackTrace(s);
        }
    }

    public void printStackTrace(PrintWriter s) {
        s.println(this);
        for (StackTraceElement ste : stackTrace) {
            s.println("\tat " + ste);
        }
        Throwable ourCause = getCause();
        if (ourCause != null) {
            s.println("Caused by: " + ourCause);
            ourCause.printStackTrace(s);
        }
    }

    public final synchronized void addSuppressed(Throwable exception) {
        if (exception == this) throw new IllegalArgumentException("Self-suppression not permitted");
        if (suppressedExceptions == null) {
            suppressedExceptions = new ArrayList<>();
        }
        suppressedExceptions.add(exception);
    }

    public final synchronized Throwable[] getSuppressed() {
        if (suppressedExceptions == null) {
            return new Throwable[0];
        }
        return suppressedExceptions.toArray(new Throwable[0]);
    }
}
