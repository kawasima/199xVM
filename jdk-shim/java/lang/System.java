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

import java.io.InputStream;
import java.io.PrintStream;
import java.util.Properties;

public final class System {
    // These are resolved natively by the VM (resolve_static_field).
    public static InputStream in = InputStream.nullInputStream();
    public static PrintStream out;
    public static PrintStream err;
    private static final Properties props = initProperties();

    private System() {}

    public static String getProperty(String key) {
        return getProperties().getProperty(key);
    }

    public static String getProperty(String key, String def) {
        return getProperties().getProperty(key, def);
    }

    public static Properties getProperties() {
        return props;
    }

    public static String lineSeparator() {
        return getProperty("line.separator", "\n");
    }

    public static native void arraycopy(Object src, int srcPos, Object dest, int destPos, int length);
    public static native long currentTimeMillis();
    public static native long nanoTime();
    public static native int identityHashCode(Object x);

    private static Properties initProperties() {
        Properties properties = new Properties();
        properties.setProperty("line.separator", "\n");
        properties.setProperty("file.separator", "/");
        properties.setProperty("path.separator", ":");
        properties.setProperty("java.version", "25");
        properties.setProperty("file.encoding", "UTF-8");
        properties.setProperty("native.encoding", "UTF-8");
        properties.setProperty("sun.jnu.encoding", "UTF-8");
        properties.setProperty("user.dir", ".");
        properties.setProperty("clojure.read.eval", "true");
        return properties;
    }
}
