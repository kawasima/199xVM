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
    public static PrintStream out;
    public static PrintStream err;
    public static InputStream in;

    private static Properties props;

    private System() {}

    public static native void arraycopy(Object src, int srcPos, Object dest, int destPos, int length);
    public static native long currentTimeMillis();
    public static native long nanoTime();
    public static native int identityHashCode(Object x);

    public static Properties getProperties() {
        if (props == null) {
            props = new Properties();
            initProperties(props);
        }
        return props;
    }

    public static String getProperty(String key) {
        return getProperties().getProperty(key);
    }

    public static String getProperty(String key, String def) {
        return getProperties().getProperty(key, def);
    }

    public static String setProperty(String key, String value) {
        return (String) getProperties().setProperty(key, value);
    }

    public static String clearProperty(String key) {
        return (String) getProperties().remove(key);
    }

    public static void setProperties(Properties p) {
        if (p == null) {
            props = new Properties();
            initProperties(props);
        } else {
            props = p;
        }
    }

    private static native void initProperties(Properties props);

    public static String lineSeparator() {
        return "\n";
    }

    public static void exit(int status) {
        throw new RuntimeException("System.exit(" + status + ")");
    }

    public static void gc() {
        // no-op
    }

    public static SecurityManager getSecurityManager() {
        return null;
    }

    public static String getenv(String name) {
        return null;
    }
}
