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
import java.io.OutputStream;
import java.io.PrintStream;
import java.io.PrintWriter;
import java.io.Reader;
import java.io.Writer;
import java.util.function.BiConsumer;
import java.util.function.BiFunction;
import java.util.function.Function;

/**
 * The {@code Properties} class represents a persistent set of
 * properties. The {@code Properties} can be saved to a stream
 * or loaded from a stream. Each key and its corresponding value
 * in the property list is a string.
 * <p>
 * A property list can contain another property list as its
 * "defaults"; this second property list is searched if
 * the property key is not found in the original property list.
 * <p>
 * Because {@code Properties} inherits from {@code Hashtable}, the
 * {@code put} and {@code putAll} methods can be applied to a
 * {@code Properties} object. Their use is strongly discouraged as they
 * allow the caller to insert entries whose keys or values are not
 * {@code Strings}. The {@code setProperty} method should be used
 * instead. If the {@code store} or {@code save} method is called
 * on a "compromised" {@code Properties} object that contains a
 * non-{@code String} key or value, the call will fail.
 *
 * <p>
 * The iterators returned by the {@code iterator} method of this class's
 * "collection views" (that is, {@code entrySet()}, {@code keySet()}, and
 * {@code values()}) may not fail-fast (unlike the Hashtable implementation).
 * These iterators are guaranteed to traverse elements as they existed upon
 * construction exactly once, and may (but are not guaranteed to) reflect any
 * modifications subsequent to construction.
 *
 * @apiNote
 * The {@code Properties} class does not inherit the concept of a load factor
 * from its superclass, {@code Hashtable}.
 *
 * @author  Arthur van Hoff
 * @author  Michael McCloskey
 * @author  Xueming Shen
 * @since   1.0
 */
public class Properties extends HashMap<Object,Object> {

    /**
     * A property list that contains default values for any keys not
     * found in this property list.
     */
    protected volatile Properties defaults;

    /**
     * Creates an empty property list with no default values.
     */
    public Properties() {
        this(null, 8);
    }

    /**
     * Creates an empty property list with no default values, and with an
     * initial size accommodating the specified number of elements without the
     * need to dynamically resize.
     *
     * @param  initialCapacity the {@code Properties} will be sized to
     *         accommodate this many elements
     * @throws IllegalArgumentException if the initial capacity of is less
     *         than zero.
     */
    public Properties(int initialCapacity) {
        this(null, initialCapacity);
    }

    /**
     * Creates an empty property list with the specified defaults.
     *
     * @param   defaults   the defaults.
     */
    public Properties(Properties defaults) {
        this(defaults, 8);
    }

    private Properties(Properties defaults, int initialCapacity) {
        super(initialCapacity);
        this.defaults = defaults;
    }

    /**
     * Calls the {@code Hashtable} method {@code put}. Provided for
     * parallelism with the {@code getProperty} method. Enforces use of
     * strings for property keys and values. The value returned is the
     * result of the {@code Hashtable} call to {@code put}.
     *
     * @param key the key to be placed into this property list.
     * @param value the value corresponding to {@code key}.
     * @return     the previous value of the specified key in this property
     *             list, or {@code null} if it did not have one.
     * @see #getProperty
     * @since    1.2
     */
    public synchronized Object setProperty(String key, String value) {
        return put(key, value);
    }

    /**
     * Reads a property list (key and element pairs) from the input
     * character stream in a simple line-oriented format.
     * <p>
     * Properties are processed in terms of lines. There are two
     * kinds of lines, <i>natural lines</i> and <i>logical lines</i>.
     * A natural line is defined as a line of
     * characters that is terminated either by a set of line terminator
     * characters ({@code \n} or {@code \r} or {@code \r\n})
     * or by the end of the stream. A natural line may be either a blank line,
     * a comment line, or hold all or some of a key-element pair.
     * A logical line holds all the data of a key-element pair, which may
     * be spread out across several adjacent natural lines by escaping
     * the line terminator sequence with a backslash character
     * {@code \}.
     *
     * @param   reader   the input character stream.
     * @throws  IOException  if an error occurred when reading from the
     *          input stream.
     * @throws  IllegalArgumentException if a malformed Unicode escape
     *          appears in the input.
     * @throws  NullPointerException if {@code reader} is null.
     * @since   1.6
     */
    public synchronized void load(Reader reader) throws IOException {
        Objects.requireNonNull(reader, "reader");
        loadFromReader(reader);
    }

    /**
     * Reads a property list (key and element pairs) from the input
     * byte stream. The input stream is in a simple line-oriented
     * format as specified in
     * {@link #load(java.io.Reader) load(Reader)} and is assumed to use
     * the ISO 8859-1 character encoding.
     *
     * @param      inStream   the input stream.
     * @throws     IOException  if an error occurred when reading from the
     *             input stream.
     * @throws     IllegalArgumentException if the input stream contains a
     *             malformed Unicode escape sequence.
     * @throws     NullPointerException if {@code inStream} is null.
     * @since 1.2
     */
    public synchronized void load(InputStream inStream) throws IOException {
        Objects.requireNonNull(inStream, "inStream");
        // Simplified: read bytes as ISO-8859-1 characters
        StringBuilder sb = new StringBuilder();
        int b;
        while ((b = inStream.read()) != -1) {
            sb.append((char) b);
        }
        // Parse the accumulated string
        loadFromString(sb.toString());
    }

    private void loadFromReader(Reader reader) throws IOException {
        StringBuilder sb = new StringBuilder();
        int ch;
        while ((ch = reader.read()) != -1) {
            sb.append((char) ch);
        }
        loadFromString(sb.toString());
    }

    /**
     * Simplified line-based key=value parser.
     * Supports:
     *   - comment lines starting with # or !
     *   - blank lines
     *   - key=value, key:value, key value separators
     *   - line continuation with trailing backslash
     *   - unicode escapes \\uXXXX in keys and values
     */
    private void loadFromString(String data) {
        String[] lines = splitLines(data);
        int i = 0;
        while (i < lines.length) {
            String line = lines[i++];

            // Skip blank lines and comment lines
            String trimmed = ltrim(line);
            if (trimmed.isEmpty()) continue;
            char first = trimmed.charAt(0);
            if (first == '#' || first == '!') continue;

            // Handle line continuation
            while (endsWithOddBackslashes(line) && i < lines.length) {
                // Remove trailing backslash, append next line (left-trimmed)
                line = line.substring(0, line.length() - 1) + ltrim(lines[i++]);
            }

            // Parse key and value
            parseKeyValue(line);
        }
    }

    private String[] splitLines(String data) {
        // Split on \n, \r\n, or \r
        java.util.ArrayList<String> result = new java.util.ArrayList<>();
        int len = data.length();
        int start = 0;
        for (int i = 0; i < len; i++) {
            char c = data.charAt(i);
            if (c == '\r') {
                result.add(data.substring(start, i));
                if (i + 1 < len && data.charAt(i + 1) == '\n') {
                    i++;
                }
                start = i + 1;
            } else if (c == '\n') {
                result.add(data.substring(start, i));
                start = i + 1;
            }
        }
        if (start <= len) {
            result.add(data.substring(start));
        }
        return result.toArray(new String[0]);
    }

    private static String ltrim(String s) {
        int i = 0;
        while (i < s.length() && (s.charAt(i) == ' ' || s.charAt(i) == '\t' || s.charAt(i) == '\f')) {
            i++;
        }
        return s.substring(i);
    }

    private static boolean endsWithOddBackslashes(String line) {
        int count = 0;
        for (int i = line.length() - 1; i >= 0 && line.charAt(i) == '\\'; i--) {
            count++;
        }
        return (count % 2) == 1;
    }

    private void parseKeyValue(String line) {
        int len = line.length();
        // Skip leading whitespace
        int keyStart = 0;
        while (keyStart < len) {
            char c = line.charAt(keyStart);
            if (c != ' ' && c != '\t' && c != '\f') break;
            keyStart++;
        }

        // Parse key (may contain escape sequences)
        StringBuilder key = new StringBuilder();
        int i = keyStart;
        boolean precedingBackslash = false;
        while (i < len) {
            char c = line.charAt(i);
            if (precedingBackslash) {
                key.append(unescapeChar(c, line, i));
                if (c == 'u') {
                    // Unicode escape: skip 4 hex digits
                    i += 4;
                }
                precedingBackslash = false;
            } else {
                if (c == '\\') {
                    precedingBackslash = true;
                } else if (c == '=' || c == ':' || c == ' ' || c == '\t' || c == '\f') {
                    break;
                } else {
                    key.append(c);
                }
            }
            i++;
        }

        // Skip separator (whitespace, then optionally = or :, then whitespace)
        while (i < len) {
            char c = line.charAt(i);
            if (c != ' ' && c != '\t' && c != '\f') break;
            i++;
        }
        if (i < len) {
            char c = line.charAt(i);
            if (c == '=' || c == ':') {
                i++;
                // Skip whitespace after separator
                while (i < len) {
                    char c2 = line.charAt(i);
                    if (c2 != ' ' && c2 != '\t' && c2 != '\f') break;
                    i++;
                }
            }
        }

        // Parse value
        StringBuilder value = new StringBuilder();
        precedingBackslash = false;
        while (i < len) {
            char c = line.charAt(i);
            if (precedingBackslash) {
                value.append(unescapeChar(c, line, i));
                if (c == 'u') {
                    i += 4;
                }
                precedingBackslash = false;
            } else {
                if (c == '\\') {
                    precedingBackslash = true;
                } else {
                    value.append(c);
                }
            }
            i++;
        }

        put(key.toString(), value.toString());
    }

    private static char unescapeChar(char c, String line, int pos) {
        switch (c) {
            case 't': return '\t';
            case 'n': return '\n';
            case 'r': return '\r';
            case 'f': return '\f';
            case 'u':
                if (pos + 4 < line.length()) {
                    String hex = line.substring(pos + 1, pos + 5);
                    try {
                        return (char) Integer.parseInt(hex, 16);
                    } catch (NumberFormatException e) {
                        return c;
                    }
                }
                return c;
            default: return c;
        }
    }

    /**
     * Writes this property list (key and element pairs) in this
     * {@code Properties} table to the output character stream in a
     * format suitable for using the {@link #load(java.io.Reader) load(Reader)}
     * method.
     *
     * @param   writer      an output character stream writer.
     * @param   comments   a description of the property list.
     * @throws  IOException if writing this property list to the specified
     *          output stream throws an {@code IOException}.
     * @throws  ClassCastException  if this {@code Properties} object
     *          contains any keys or values that are not {@code Strings}.
     * @throws  NullPointerException  if {@code writer} is null.
     * @since   1.6
     */
    public void store(Writer writer, String comments)
        throws IOException
    {
        storeImpl(writer, comments, false);
    }

    /**
     * Writes this property list (key and element pairs) in this
     * {@code Properties} table to the output stream in a format suitable
     * for loading into a {@code Properties} table using the
     * {@link #load(InputStream) load(InputStream)} method.
     *
     * @param   out      an output stream.
     * @param   comments   a description of the property list.
     * @throws  IOException if writing this property list to the specified
     *          output stream throws an {@code IOException}.
     * @throws  ClassCastException  if this {@code Properties} object
     *          contains any keys or values that are not {@code Strings}.
     * @throws  NullPointerException if {@code out} is null.
     * @since   1.2
     */
    public void store(OutputStream out, String comments)
        throws IOException
    {
        // Wrap OutputStream in a Writer-like output
        storeToOutputStream(out, comments);
    }

    private void storeImpl(Writer writer, String comments, boolean escUnicode)
        throws IOException
    {
        if (comments != null) {
            writeComments(writer, comments);
        }
        writer.write("#" + new Date().toString());
        writer.write("\n");
        for (Map.Entry<Object, Object> e : entrySet()) {
            String key = saveConvert((String) e.getKey(), true, escUnicode);
            String val = saveConvert((String) e.getValue(), false, escUnicode);
            writer.write(key + "=" + val);
            writer.write("\n");
        }
        writer.flush();
    }

    private void storeToOutputStream(OutputStream out, String comments)
        throws IOException
    {
        // Write as ISO-8859-1
        if (comments != null) {
            writeCommentsToStream(out, comments);
        }
        String dateComment = "#" + new Date().toString() + "\n";
        writeISO8859(out, dateComment);
        for (Map.Entry<Object, Object> e : entrySet()) {
            String key = saveConvert((String) e.getKey(), true, true);
            String val = saveConvert((String) e.getValue(), false, true);
            writeISO8859(out, key + "=" + val + "\n");
        }
        out.flush();
    }

    private static void writeISO8859(OutputStream out, String s) throws IOException {
        for (int i = 0; i < s.length(); i++) {
            out.write(s.charAt(i) & 0xFF);
        }
    }

    private static String saveConvert(String theString, boolean escapeSpace, boolean escapeUnicode) {
        int len = theString.length();
        StringBuilder outBuffer = new StringBuilder(len * 2);
        for (int x = 0; x < len; x++) {
            char aChar = theString.charAt(x);
            if ((aChar > 61) && (aChar < 127)) {
                if (aChar == '\\') {
                    outBuffer.append('\\');
                    outBuffer.append('\\');
                    continue;
                }
                outBuffer.append(aChar);
                continue;
            }
            switch (aChar) {
                case ' ':
                    if (x == 0 || escapeSpace)
                        outBuffer.append('\\');
                    outBuffer.append(' ');
                    break;
                case '\t':
                    outBuffer.append('\\');
                    outBuffer.append('t');
                    break;
                case '\n':
                    outBuffer.append('\\');
                    outBuffer.append('n');
                    break;
                case '\r':
                    outBuffer.append('\\');
                    outBuffer.append('r');
                    break;
                case '\f':
                    outBuffer.append('\\');
                    outBuffer.append('f');
                    break;
                case '=':
                case ':':
                case '#':
                case '!':
                    outBuffer.append('\\');
                    outBuffer.append(aChar);
                    break;
                default:
                    if (((aChar < 0x0020) || (aChar > 0x007e)) && escapeUnicode) {
                        outBuffer.append('\\');
                        outBuffer.append('u');
                        outBuffer.append(toHex((aChar >> 12) & 0xF));
                        outBuffer.append(toHex((aChar >>  8) & 0xF));
                        outBuffer.append(toHex((aChar >>  4) & 0xF));
                        outBuffer.append(toHex( aChar        & 0xF));
                    } else {
                        outBuffer.append(aChar);
                    }
            }
        }
        return outBuffer.toString();
    }

    private static char toHex(int nibble) {
        return "0123456789abcdef".charAt(nibble & 0xF);
    }

    private static void writeComments(Writer writer, String comments) throws IOException {
        int len = comments.length();
        writer.write("#");
        int current = 0;
        int last = 0;
        while (current < len) {
            char c = comments.charAt(current);
            if (c == '\r' || c == '\n') {
                if (last != current)
                    writer.write(comments.substring(last, current));
                writer.write("\n");
                if (c == '\r' && current + 1 < len && comments.charAt(current + 1) == '\n') {
                    current++;
                }
                if (current + 1 < len) {
                    writer.write("#");
                }
                last = current + 1;
            }
            current++;
        }
        if (last != current)
            writer.write(comments.substring(last, current));
        writer.write("\n");
    }

    private static void writeCommentsToStream(OutputStream out, String comments) throws IOException {
        int len = comments.length();
        writeISO8859(out, "#");
        int current = 0;
        int last = 0;
        while (current < len) {
            char c = comments.charAt(current);
            if (c == '\r' || c == '\n') {
                if (last != current)
                    writeISO8859(out, comments.substring(last, current));
                writeISO8859(out, "\n");
                if (c == '\r' && current + 1 < len && comments.charAt(current + 1) == '\n') {
                    current++;
                }
                if (current + 1 < len) {
                    writeISO8859(out, "#");
                }
                last = current + 1;
            }
            current++;
        }
        if (last != current)
            writeISO8859(out, comments.substring(last, current));
        writeISO8859(out, "\n");
    }

    /**
     * Searches for the property with the specified key in this property list.
     * If the key is not found in this property list, the default property list,
     * and its defaults, recursively, are then checked. The method returns
     * {@code null} if the property is not found.
     *
     * @param   key   the property key.
     * @return  the value in this property list with the specified key value.
     * @see     #setProperty
     * @see     #defaults
     */
    public String getProperty(String key) {
        Object oval = get(key);
        String sval = (oval instanceof String) ? (String)oval : null;
        Properties parent = defaults;
        return (sval == null && parent != null) ? parent.getProperty(key) : sval;
    }

    /**
     * Searches for the property with the specified key in this property list.
     * If the key is not found in this property list, the default property list,
     * and its defaults, recursively, are then checked. The method returns the
     * default value argument if the property is not found.
     *
     * @param   key            the hashtable key.
     * @param   defaultValue   a default value.
     *
     * @return  the value in this property list with the specified key value.
     * @see     #setProperty
     * @see     #defaults
     */
    public String getProperty(String key, String defaultValue) {
        String val = getProperty(key);
        return (val == null) ? defaultValue : val;
    }

    /**
     * Returns an enumeration of all the keys in this property list,
     * including distinct keys in the default property list if a key
     * of the same name has not already been found from the main
     * properties list.
     *
     * @return  an enumeration of all the keys in this property list, including
     *          the keys in the default property list.
     * @throws  ClassCastException if any key in this property list
     *          is not a string.
     * @see     java.util.Enumeration
     * @see     java.util.Properties#defaults
     * @see     #stringPropertyNames
     */
    public Enumeration<?> propertyNames() {
        Set<String> h = new HashSet<>();
        enumerateStringProperties(h);
        return Collections.enumeration(h);
    }

    /**
     * Returns an unmodifiable set of keys from this property list
     * where the key and its corresponding value are strings,
     * including distinct keys in the default property list if a key
     * of the same name has not already been found from the main
     * properties list.  Properties whose key or value is not
     * of type {@code String} are omitted.
     * <p>
     * The returned set is not backed by this {@code Properties} object.
     * Changes to this {@code Properties} object are not reflected in the
     * returned set.
     *
     * @return  an unmodifiable set of keys in this property list where
     *          the key and its corresponding value are strings,
     *          including the keys in the default property list.
     * @see     java.util.Properties#defaults
     * @since   1.6
     */
    public Set<String> stringPropertyNames() {
        Set<String> h = new HashSet<>();
        enumerateStringProperties(h);
        return Collections.unmodifiableSet(h);
    }

    /**
     * Prints this property list out to the specified output stream.
     * This method is useful for debugging.
     *
     * @param   out   an output stream.
     * @throws  ClassCastException if any key in this property list
     *          is not a string.
     */
    public void list(PrintStream out) {
        out.println("-- listing properties --");
        Set<String> h = new HashSet<>();
        enumerateStringProperties(h);
        for (String key : h) {
            String val = getProperty(key);
            if (val.length() > 40) {
                val = val.substring(0, 37) + "...";
            }
            out.println(key + "=" + val);
        }
    }

    /**
     * Prints this property list out to the specified output stream.
     * This method is useful for debugging.
     *
     * @param   out   an output stream.
     * @throws  ClassCastException if any key in this property list
     *          is not a string.
     * @since   1.1
     */
    public void list(PrintWriter out) {
        out.println("-- listing properties --");
        Set<String> h = new HashSet<>();
        enumerateStringProperties(h);
        for (String key : h) {
            String val = getProperty(key);
            if (val.length() > 40) {
                val = val.substring(0, 37) + "...";
            }
            out.println(key + "=" + val);
        }
    }

    /**
     * Enumerates all key/value pairs into the specified Set.
     * Includes defaults recursively.
     */
    private void enumerateStringProperties(Set<String> h) {
        if (defaults != null) {
            defaults.enumerateStringProperties(h);
        }
        for (Map.Entry<Object, Object> e : entrySet()) {
            Object k = e.getKey();
            Object v = e.getValue();
            if (k instanceof String && v instanceof String) {
                h.add((String) k);
            }
        }
    }

    // --- Overrides for synchronized behavior (Hashtable compatibility) ---

    @Override
    public synchronized int size() {
        return super.size();
    }

    @Override
    public synchronized boolean isEmpty() {
        return super.isEmpty();
    }

    @Override
    public synchronized Object get(Object key) {
        return super.get(key);
    }

    @Override
    public synchronized boolean containsKey(Object key) {
        return super.containsKey(key);
    }

    @Override
    public synchronized boolean containsValue(Object value) {
        return super.containsValue(value);
    }

    @Override
    public synchronized Object put(Object key, Object value) {
        return super.put(key, value);
    }

    @Override
    public synchronized Object remove(Object key) {
        return super.remove(key);
    }

    @Override
    public synchronized void putAll(Map<?, ?> t) {
        super.putAll(t);
    }

    @Override
    public synchronized void clear() {
        super.clear();
    }

    @Override
    public synchronized String toString() {
        return super.toString();
    }

    @Override
    public synchronized Set<Object> keySet() {
        return super.keySet();
    }

    @Override
    public synchronized Collection<Object> values() {
        return super.values();
    }

    @Override
    public synchronized Set<Map.Entry<Object,Object>> entrySet() {
        return super.entrySet();
    }

    @Override
    public synchronized boolean equals(Object o) {
        return super.equals(o);
    }

    @Override
    public synchronized int hashCode() {
        return super.hashCode();
    }

    @Override
    public synchronized Object getOrDefault(Object key, Object defaultValue) {
        return super.getOrDefault(key, defaultValue);
    }

    @Override
    public synchronized void forEach(BiConsumer<? super Object, ? super Object> action) {
        super.forEach(action);
    }

    @Override
    public synchronized void replaceAll(BiFunction<? super Object, ? super Object, ?> function) {
        super.replaceAll(function);
    }

    @Override
    public synchronized Object putIfAbsent(Object key, Object value) {
        return super.putIfAbsent(key, value);
    }

    @Override
    public synchronized boolean remove(Object key, Object value) {
        return super.remove(key, value);
    }

    @Override
    public synchronized boolean replace(Object key, Object oldValue, Object newValue) {
        return super.replace(key, oldValue, newValue);
    }

    @Override
    public synchronized Object replace(Object key, Object value) {
        return super.replace(key, value);
    }

    @Override
    public synchronized Object computeIfAbsent(Object key, Function<? super Object, ?> mappingFunction) {
        return super.computeIfAbsent(key, mappingFunction);
    }

    @Override
    public synchronized Object computeIfPresent(Object key,
            BiFunction<? super Object, ? super Object, ?> remappingFunction) {
        return super.computeIfPresent(key, remappingFunction);
    }

    @Override
    public synchronized Object compute(Object key,
            BiFunction<? super Object, ? super Object, ?> remappingFunction) {
        return super.compute(key, remappingFunction);
    }

    @Override
    public synchronized Object merge(Object key, Object value,
            BiFunction<? super Object, ? super Object, ?> remappingFunction) {
        return super.merge(key, value, remappingFunction);
    }

    @Override
    public synchronized Object clone() {
        Properties clone = (Properties) super.clone();
        clone.defaults = this.defaults;
        return clone;
    }
}
