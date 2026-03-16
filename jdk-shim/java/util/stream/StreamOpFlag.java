/*
 * Copyright (c) 2012, 2020, Oracle and/or its affiliates. All rights reserved.
 * ORACLE PROPRIETARY/CONFIDENTIAL. Use is subject to license terms.
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 *
 */
package java.util.stream;

/**
 * Flags corresponding to characteristics of streams and operations.
 *
 * <p>This is a minimal shim providing only the flag constants needed by
 * {@link WhileOps}.
 *
 * @since 1.8
 */
final class StreamOpFlag {
    private StreamOpFlag() { }

    /** The stream/operation is not sized. */
    static final int NOT_SIZED = 0;

    /** The stream/operation is a short-circuit operation. */
    static final int IS_SHORT_CIRCUIT = 0;
}
