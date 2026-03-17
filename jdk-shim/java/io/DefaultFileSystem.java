/*
 * Copyright (c) 2012, 2024, Oracle and/or its affiliates. All rights reserved.
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

/**
 * 199xVM stub: returns a Unix-like stub FileSystem.
 */
class DefaultFileSystem {
    static FileSystem getFileSystem() {
        return new StubFileSystem();
    }

    private static class StubFileSystem extends FileSystem {
        public char getSeparator() { return '/'; }
        public char getPathSeparator() { return ':'; }
        public String normalize(String path) { return path; }
        public int prefixLength(String path) {
            return (path.length() > 0 && path.charAt(0) == '/') ? 1 : 0;
        }
        public String resolve(String parent, String child) {
            if (child.isEmpty()) return parent;
            if (child.charAt(0) == '/') return child;
            if (parent.equals("/")) return parent + child;
            return parent + "/" + child;
        }
        public String getDefaultParent() { return "/"; }
        public String fromURIPath(String path) { return path; }
        public boolean isAbsolute(File f) {
            return f.getPath().startsWith("/");
        }
        public String resolve(File f) { return f.getPath(); }
        public String canonicalize(String path) { return path; }
        public int getBooleanAttributes(File f) { return 0; }
        public boolean checkAccess(File f, int access) { return false; }
        public boolean setPermission(File f, int access, boolean enable, boolean owneronly) { return false; }
        public long getLastModifiedTime(File f) { return 0L; }
        public long getLength(File f) { return 0L; }
        public boolean createFileExclusively(String pathname) { return false; }
        public boolean delete(File f) { return false; }
        public String[] list(File f) { return null; }
        public boolean createDirectory(File f) { return false; }
        public boolean rename(File f1, File f2) { return false; }
        public boolean setLastModifiedTime(File f, long time) { return false; }
        public boolean setReadOnly(File f) { return false; }
        public File[] listRoots() { return new File[]{new File("/")}; }
        public long getSpace(File f, int t) { return 0L; }
        public int getNameMax(String path) { return 255; }
        public int compare(File f1, File f2) { return f1.getPath().compareTo(f2.getPath()); }
        public int hashCode(File f) { return f.getPath().hashCode(); }
    }
}
