/*
 * Copyright (c) 2012, 2024, Oracle and/or its affiliates. All rights reserved.
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
 * Factory methods for constructing implementations of {@link Node} and
 * {@link Node.Builder} and their primitive specializations.
 *
 * <p>This is a minimal shim providing only the constants needed by
 * {@link SpinedBuffer}.
 *
 * @since 1.8
 */
final class Nodes {
    private Nodes() {
        throw new Error("no instances");
    }

    /**
     * The maximum size of an array that can usually be allocated.
     */
    static final int MAX_ARRAY_SIZE = Integer.MAX_VALUE - 8;

    static final String BAD_SIZE = "Stream size exceeds max array size";
}
