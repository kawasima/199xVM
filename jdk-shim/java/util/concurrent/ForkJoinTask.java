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

import java.io.Serializable;
import java.util.Collection;

public abstract class ForkJoinTask<V> implements Future<V>, Serializable {
    private static final long serialVersionUID = -7721805057305804111L;
    private static final ForkJoinPool COMMON = ForkJoinPool.commonPool();

    private transient Throwable exception;
    private transient boolean done;
    private transient boolean cancelled;
    private transient short tag;

    protected ForkJoinTask() {}

    public abstract V getRawResult();
    protected abstract void setRawResult(V value);
    protected abstract boolean exec();

    private V doExec() {
        if (!done) {
            try {
                if (!cancelled) {
                    exec();
                }
            } catch (Throwable t) {
                exception = t;
            } finally {
                done = true;
            }
        }
        if (cancelled) {
            throw new CancellationException();
        }
        if (exception != null) {
            if (exception instanceof RuntimeException re) throw re;
            if (exception instanceof Error e) throw e;
            throw new RuntimeException(exception);
        }
        return getRawResult();
    }

    public final V invoke() {
        return doExec();
    }

    public final ForkJoinTask<V> fork() {
        return this;
    }

    public final V join() {
        return doExec();
    }

    public static void invokeAll(ForkJoinTask<?> t1, ForkJoinTask<?> t2) {
        t1.fork();
        t2.invoke();
        t1.join();
    }

    public static void invokeAll(ForkJoinTask<?>... tasks) {
        for (ForkJoinTask<?> task : tasks) task.fork();
        for (ForkJoinTask<?> task : tasks) task.join();
    }

    public static <T extends ForkJoinTask<?>> Collection<T> invokeAll(Collection<T> tasks) {
        for (T task : tasks) task.fork();
        for (T task : tasks) task.join();
        return tasks;
    }

    public boolean cancel(boolean mayInterruptIfRunning) {
        if (done) return false;
        cancelled = true;
        done = true;
        return true;
    }

    public final boolean isDone() {
        return done;
    }

    public final boolean isCancelled() {
        return cancelled;
    }

    public final boolean isCompletedAbnormally() {
        return done && (cancelled || exception != null);
    }

    public final boolean isCompletedNormally() {
        return done && !cancelled && exception == null;
    }

    public Future.State state() {
        if (!done) return Future.State.RUNNING;
        if (cancelled) return Future.State.CANCELLED;
        if (exception != null) return Future.State.FAILED;
        return Future.State.SUCCESS;
    }

    public V resultNow() {
        if (!done || cancelled || exception != null) {
            throw new IllegalStateException();
        }
        return getRawResult();
    }

    public Throwable exceptionNow() {
        if (!done) throw new IllegalStateException();
        return exception;
    }

    public final Throwable getException() {
        if (cancelled) return new CancellationException();
        return exception;
    }

    public void completeExceptionally(Throwable ex) {
        exception = ex;
        done = true;
    }

    public void complete(V value) {
        setRawResult(value);
        done = true;
    }

    public final void quietlyComplete() {
        done = true;
    }

    public final V get() throws InterruptedException, ExecutionException {
        try {
            return doExec();
        } catch (CancellationException e) {
            throw e;
        } catch (RuntimeException e) {
            Throwable cause = e.getCause();
            throw new ExecutionException(cause != null ? cause : e);
        }
    }

    public final V get(long timeout, TimeUnit unit)
        throws InterruptedException, ExecutionException, TimeoutException {
        return get();
    }

    public final void quietlyJoin() {
        doExec();
    }

    public final void quietlyInvoke() {
        doExec();
    }

    public final boolean quietlyJoin(long timeout, TimeUnit unit) throws InterruptedException {
        quietlyJoin();
        return true;
    }

    public final boolean quietlyJoinUninterruptibly(long timeout, TimeUnit unit) {
        quietlyJoin();
        return true;
    }

    public static void helpQuiesce() {}

    public void reinitialize() {
        exception = null;
        done = false;
        cancelled = false;
    }

    public static ForkJoinPool getPool() {
        return COMMON;
    }

    public static boolean inForkJoinPool() {
        return false;
    }

    public boolean tryUnfork() {
        return false;
    }

    public static int getQueuedTaskCount() {
        return 0;
    }

    public static int getSurplusQueuedTaskCount() {
        return 0;
    }

    protected static ForkJoinTask<?> peekNextLocalTask() { return null; }
    protected static ForkJoinTask<?> pollNextLocalTask() { return null; }
    protected static ForkJoinTask<?> pollTask() { return null; }
    protected static ForkJoinTask<?> pollSubmission() { return null; }

    public final short getForkJoinTaskTag() {
        return tag;
    }

    public final short setForkJoinTaskTag(short newValue) {
        short prev = tag;
        tag = newValue;
        return prev;
    }

    public final boolean compareAndSetForkJoinTaskTag(short expect, short update) {
        if (tag != expect) return false;
        tag = update;
        return true;
    }

    public static ForkJoinTask<?> adapt(Runnable runnable) {
        return adapt(runnable, null);
    }

    public static <T> ForkJoinTask<T> adapt(Runnable runnable, T result) {
        return new AdaptedRunnableAction<>(runnable, result);
    }

    public static <T> ForkJoinTask<T> adapt(Callable<? extends T> callable) {
        return new AdaptedCallable<>(callable);
    }

    public static <T> ForkJoinTask<T> adaptInterruptible(Callable<? extends T> callable) {
        return adapt(callable);
    }

    public static <T> ForkJoinTask<T> adaptInterruptible(Runnable runnable, T result) {
        return adapt(runnable, result);
    }

    public static ForkJoinTask<?> adaptInterruptible(Runnable runnable) {
        return adapt(runnable);
    }

    private static final class AdaptedRunnableAction<T> extends ForkJoinTask<T> {
        private final Runnable runnable;
        private T result;

        private AdaptedRunnableAction(Runnable runnable, T result) {
            this.runnable = runnable;
            this.result = result;
        }

        public T getRawResult() { return result; }
        protected void setRawResult(T value) { result = value; }
        protected boolean exec() {
            runnable.run();
            return true;
        }
    }

    private static final class AdaptedCallable<T> extends ForkJoinTask<T> {
        private final Callable<? extends T> callable;
        private T result;

        private AdaptedCallable(Callable<? extends T> callable) {
            this.callable = callable;
        }

        public T getRawResult() { return result; }
        protected void setRawResult(T value) { result = value; }
        protected boolean exec() {
            try {
                result = callable.call();
                return true;
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        }
    }
}
