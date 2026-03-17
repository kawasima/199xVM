/*
 * Copyright (c) 1994, 2025, Oracle and/or its affiliates. All rights reserved.
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

package java.io;

import java.net.URI;
import java.net.URISyntaxException;

public class File implements Serializable, Comparable<File> {
    private static final long serialVersionUID = 301077366599181567L;

    public static final char separatorChar = '/';
    public static final String separator = "/";
    public static final char pathSeparatorChar = ':';
    public static final String pathSeparator = ":";

    private final String path;

    public File(String pathname) {
        this.path = normalize(pathname);
    }

    public File(String parent, String child) {
        this(parent == null ? child : join(normalize(parent), normalize(child)));
    }

    public File(File parent, String child) {
        this(parent == null ? child : join(parent.getPath(), normalize(child)));
    }

    private static String normalize(String path) {
        if (path == null || path.length() == 0) {
            return "";
        }
        return path.replace('\\', separatorChar);
    }

    private static String join(String parent, String child) {
        if (parent == null || parent.length() == 0) {
            return child == null ? "" : child;
        }
        if (child == null || child.length() == 0) {
            return parent;
        }
        if (child.charAt(0) == separatorChar) {
            return child;
        }
        return parent.endsWith(separator) ? parent + child : parent + separator + child;
    }

    public String getPath() {
        return path;
    }

    public String getName() {
        int idx = path.lastIndexOf(separatorChar);
        return idx >= 0 ? path.substring(idx + 1) : path;
    }

    public String getParent() {
        int idx = path.lastIndexOf(separatorChar);
        if (idx <= 0) {
            return idx == 0 ? separator : null;
        }
        return path.substring(0, idx);
    }

    public File getParentFile() {
        String parent = getParent();
        return parent == null ? null : new File(parent);
    }

    public boolean isAbsolute() {
        return path.startsWith(separator);
    }

    public String getAbsolutePath() {
        return isAbsolute() ? path : separator + path;
    }

    public File getAbsoluteFile() {
        return new File(getAbsolutePath());
    }

    public String getCanonicalPath() throws IOException {
        return getAbsolutePath();
    }

    public File getCanonicalFile() throws IOException {
        return getAbsoluteFile();
    }

    public boolean exists() {
        return false;
    }

    public boolean isFile() {
        return !path.endsWith(separator);
    }

    public boolean isDirectory() {
        return path.endsWith(separator);
    }

    public boolean mkdirs() {
        return true;
    }

    public boolean delete() {
        return false;
    }

    public URI toURI() {
        try {
            return new URI("file:" + getAbsolutePath());
        } catch (URISyntaxException e) {
            throw new IllegalArgumentException(e.getMessage());
        }
    }

    public int compareTo(File pathname) {
        return getPath().compareTo(pathname.getPath());
    }

    public int hashCode() {
        return path.hashCode();
    }

    public boolean equals(Object obj) {
        return obj instanceof File other && path.equals(other.path);
    }

    public String toString() {
        return path;
    }
}
