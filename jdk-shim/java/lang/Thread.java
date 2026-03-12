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

public class Thread implements Runnable {
    @FunctionalInterface
    public static interface UncaughtExceptionHandler {
        void uncaughtException(Thread t, Throwable e);
    }

    private static int nextId = 1;
    private static final Thread MAIN = new Thread("main");

    private final int tid;
    private String name;
    private int priority = 5;
    private boolean daemon;
    private Runnable target;
    private ClassLoader contextClassLoader;

    public Thread() {
        this((String) null, (Runnable) null);
    }

    public Thread(Runnable target) {
        this(null, target);
    }

    public Thread(String name) {
        this(name, null);
    }

    public Thread(Runnable target, String name) {
        this(name, target);
    }

    private Thread(String name, Runnable target) {
        this.tid = nextId++;
        this.name = (name == null) ? ("Thread-" + this.tid) : name;
        this.target = target;
    }

    public static Thread currentThread() {
        return MAIN;
    }

    public static void yield() {}

    public static void sleep(long millis) throws InterruptedException {}

    public static void sleep(long millis, int nanos) throws InterruptedException {}

    public final void join() throws InterruptedException {}

    public final void join(long millis) throws InterruptedException {}

    public final void join(long millis, int nanos) throws InterruptedException {}

    public void run() {
        if (target != null) {
            target.run();
        }
    }

    public synchronized void start() {
        run();
    }

    public final void setName(String name) {
        if (name == null) throw new NullPointerException();
        this.name = name;
    }

    public final String getName() {
        return name;
    }

    public final long getId() {
        return (long) tid;
    }

    public final int getPriority() {
        return priority;
    }

    public final void setPriority(int newPriority) {
        this.priority = newPriority;
    }

    public final boolean isDaemon() {
        return daemon;
    }

    public final void setDaemon(boolean on) {
        this.daemon = on;
    }

    public final ClassLoader getContextClassLoader() {
        return contextClassLoader;
    }

    public void setContextClassLoader(ClassLoader cl) {
        this.contextClassLoader = cl;
    }

    public static boolean interrupted() {
        return false;
    }

    public boolean isInterrupted() {
        return false;
    }

    public void interrupt() {}

    public boolean isAlive() {
        return false;
    }

    public final boolean isVirtual() {
        return false;
    }

    public String toString() {
        return "Thread[" + name + "," + priority + ",main]";
    }
}
