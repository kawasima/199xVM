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

public class ExecutorCompletionService<V> implements CompletionService<V> {
    private final Executor executor;
    private final BlockingQueue<Future<V>> completionQueue;

    public ExecutorCompletionService(Executor executor) {
        this(executor, new LinkedBlockingQueue<>());
    }

    public ExecutorCompletionService(Executor executor, BlockingQueue<Future<V>> completionQueue) {
        if (executor == null || completionQueue == null) {
            throw new NullPointerException();
        }
        this.executor = executor;
        this.completionQueue = completionQueue;
    }

    public Future<V> submit(Callable<V> task) {
        if (task == null) throw new NullPointerException();
        Future<V> f;
        if (executor instanceof ExecutorService es) {
            f = es.submit(task);
        } else {
            FutureTask<V> ft = new FutureTask<>(task);
            executor.execute(ft);
            f = ft;
        }
        completionQueue.offer(f);
        return f;
    }

    public Future<V> submit(Runnable task, V result) {
        if (task == null) throw new NullPointerException();
        Future<V> f;
        if (executor instanceof ExecutorService es) {
            f = es.submit(task, result);
        } else {
            FutureTask<V> ft = new FutureTask<>(task, result);
            executor.execute(ft);
            f = ft;
        }
        completionQueue.offer(f);
        return f;
    }

    public Future<V> take() throws InterruptedException {
        return completionQueue.take();
    }

    public Future<V> poll() {
        return completionQueue.poll();
    }

    public Future<V> poll(long timeout, TimeUnit unit) throws InterruptedException {
        return completionQueue.poll(timeout, unit);
    }
}
