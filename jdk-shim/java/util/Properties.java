/*
 * Copyright (c) 1995, 2025, Oracle and/or its affiliates. All rights reserved.
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

package java.util;

import java.io.IOException;
import java.io.InputStream;

public class Properties extends HashMap<Object, Object> {
    protected Properties defaults;

    public Properties() {
        this(null);
    }

    public Properties(Properties defaults) {
        this.defaults = defaults;
    }

    public Object setProperty(String key, String value) {
        return put(key, value);
    }

    public String getProperty(String key) {
        Object value = get(key);
        if (value instanceof String s) {
            return s;
        }
        return defaults == null ? null : defaults.getProperty(key);
    }

    public String getProperty(String key, String defaultValue) {
        String value = getProperty(key);
        return value == null ? defaultValue : value;
    }

    public java.util.Set<String> stringPropertyNames() {
        java.util.HashSet<String> names = new java.util.HashSet<>();
        for (Object key : keySet()) {
            if (key instanceof String s) {
                names.add(s);
            }
        }
        if (defaults != null) {
            names.addAll(defaults.stringPropertyNames());
        }
        return names;
    }

    public synchronized void load(InputStream inStream) throws IOException {
        if (inStream == null) {
            throw new NullPointerException();
        }
        byte[] bytes = inStream.readAllBytes();
        String text = new String(bytes);
        String[] lines = text.split("\n");
        for (int i = 0; i < lines.length; i++) {
            String line = lines[i];
            if (line.endsWith("\r")) {
                line = line.substring(0, line.length() - 1);
            }
            line = line.trim();
            if (line.length() == 0 || line.startsWith("#") || line.startsWith("!")) {
                continue;
            }
            int sep = line.indexOf('=');
            if (sep < 0) {
                sep = line.indexOf(':');
            }
            if (sep < 0) {
                put(line, "");
                continue;
            }
            String key = line.substring(0, sep).trim();
            String value = line.substring(sep + 1).trim();
            put(key, value);
        }
    }
}
