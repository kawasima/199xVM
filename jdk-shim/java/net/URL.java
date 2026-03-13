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

package java.net;

import java.io.IOException;
import java.io.InputStream;

/**
 * Class {@code URL} represents a Uniform Resource Locator, a pointer to a
 * "resource" on the World Wide Web.
 *
 * <p>This shim implementation delegates parsing to {@link URI} and does not
 * support network I/O. {@link #openConnection()} and {@link #openStream()}
 * always throw {@link UnsupportedOperationException}.
 *
 * @author  James Gosling
 * @since   1.0
 */
public final class URL implements java.io.Serializable {

    @java.io.Serial
    private static final long serialVersionUID = -7627629688361524110L;

    private final String protocol;
    private final String host;
    private final int port;
    private final String file;
    private final String ref;

    /**
     * Creates a {@code URL} object from the {@code String} representation.
     *
     * @param   spec   the {@code String} to parse as a URL.
     * @throws  MalformedURLException  if no protocol is specified, or an
     *          unknown protocol is found, or the parsed URL fails to comply
     *          with the specific syntax of the associated protocol.
     */
    public URL(String spec) throws MalformedURLException {
        try {
            URI uri = new URI(spec);
            this.protocol = uri.getScheme();
            if (this.protocol == null) throw new MalformedURLException("no protocol: " + spec);
            this.host = uri.getHost() != null ? uri.getHost() : "";
            this.port = uri.getPort();
            String path = uri.getRawPath() != null ? uri.getRawPath() : "";
            String query = uri.getRawQuery();
            this.file = query != null ? path + "?" + query : path;
            this.ref = uri.getFragment();
        } catch (URISyntaxException e) {
            throw new MalformedURLException(e.getMessage());
        }
    }

    /**
     * Creates a URL from the specified {@code protocol}, {@code host},
     * {@code port} number, and {@code file}.
     *
     * @param   protocol   the name of the protocol to use.
     * @param   host       the name of the host.
     * @param   port       the port number on the host.
     * @param   file       the file on the host.
     * @throws  MalformedURLException  if an unknown protocol or the port
     *          is a negative number other than -1.
     */
    public URL(String protocol, String host, int port, String file)
            throws MalformedURLException {
        if (protocol == null) throw new MalformedURLException("null protocol");
        this.protocol = protocol.toLowerCase();
        this.host = host != null ? host : "";
        this.port = port;
        this.file = file != null ? file : "";
        this.ref = null;
    }

    /**
     * Creates a URL from the specified {@code protocol}, {@code host},
     * and {@code file} on the host. The default port for the specified
     * protocol is used.
     *
     * @param   protocol   the name of the protocol to use.
     * @param   host       the name of the host.
     * @param   file       the file on the host.
     * @throws  MalformedURLException  if an unknown protocol is specified.
     */
    public URL(String protocol, String host, String file) throws MalformedURLException {
        this(protocol, host, -1, file);
    }

    /**
     * Creates a URL by parsing the given spec within a specified context.
     *
     * @param   context   the context in which to parse the specification.
     * @param   spec      the {@code String} to parse as a URL.
     * @throws  MalformedURLException  if no protocol is specified, or an
     *          unknown protocol is found, or the parsed URL fails to comply.
     */
    public URL(URL context, String spec) throws MalformedURLException {
        try {
            URI base = (context != null) ? context.toURI() : null;
            URI resolved = (base != null) ? base.resolve(new URI(spec)) : new URI(spec);
            URL result = new URL(resolved.toString());
            this.protocol = result.protocol;
            this.host = result.host;
            this.port = result.port;
            this.file = result.file;
            this.ref = result.ref;
        } catch (URISyntaxException e) {
            throw new MalformedURLException(e.getMessage());
        }
    }

    /**
     * Gets the protocol name of this {@code URL}.
     *
     * @return  the protocol of this {@code URL}.
     */
    public String getProtocol() {
        return protocol;
    }

    /**
     * Gets the host name of this {@code URL}, if applicable.
     *
     * @return  the host name of this {@code URL}.
     */
    public String getHost() {
        return host;
    }

    /**
     * Gets the port number of this {@code URL}.
     *
     * @return  the port number, or -1 if the port is not set.
     */
    public int getPort() {
        return port;
    }

    /**
     * Gets the default port number of the protocol associated with this
     * {@code URL}.
     *
     * @return  the default port number, or -1 if unknown.
     */
    public int getDefaultPort() {
        if ("http".equals(protocol)) return 80;
        if ("https".equals(protocol)) return 443;
        if ("ftp".equals(protocol)) return 21;
        return -1;
    }

    /**
     * Gets the file name of this {@code URL}.
     *
     * @return  the file name of this {@code URL}, or an empty string if one
     *          does not exist.
     */
    public String getFile() {
        return file;
    }

    /**
     * Gets the path part of this {@code URL}.
     *
     * @return  the path part of this {@code URL}, or an empty string if one
     *          does not exist.
     */
    public String getPath() {
        int q = file.indexOf('?');
        return q >= 0 ? file.substring(0, q) : file;
    }

    /**
     * Gets the query part of this {@code URL}.
     *
     * @return  the query part of this {@code URL}, or {@code null} if one
     *          does not exist.
     */
    public String getQuery() {
        int q = file.indexOf('?');
        return q >= 0 ? file.substring(q + 1) : null;
    }

    /**
     * Gets the anchor (also known as the "reference") of this {@code URL}.
     *
     * @return  the anchor (also known as the "reference") of this
     *          {@code URL}, or {@code null} if one does not exist.
     */
    public String getRef() {
        return ref;
    }

    /**
     * Gets the authority part of this {@code URL}.
     *
     * @return  the authority part of this {@code URL}.
     */
    public String getAuthority() {
        if (host == null || host.isEmpty()) return null;
        return port >= 0 ? host + ":" + port : host;
    }

    /**
     * Gets the userInfo part of this {@code URL}.
     *
     * @return  the userInfo part of this {@code URL}, or {@code null}
     *          if one does not exist.
     */
    public String getUserInfo() {
        return null;
    }

    /**
     * Returns a {@link URI} equivalent to this URL.
     *
     * @return  a URI instance equivalent to this URL.
     * @throws  URISyntaxException if this URL cannot be converted to a URI.
     */
    public URI toURI() throws URISyntaxException {
        return new URI(toExternalForm());
    }

    /**
     * Opens a connection to this {@code URL}.
     *
     * <p>Network I/O is not supported in 199xVM.
     *
     * @throws UnsupportedOperationException always.
     */
    public java.net.URLConnection openConnection() throws IOException {
        throw new UnsupportedOperationException("Network I/O not supported in 199xVM");
    }

    /**
     * Opens a connection to this {@code URL} and returns an {@code InputStream}.
     *
     * <p>Network I/O is not supported in 199xVM.
     *
     * @throws UnsupportedOperationException always.
     */
    public InputStream openStream() throws IOException {
        throw new UnsupportedOperationException("Network I/O not supported in 199xVM");
    }

    /**
     * Constructs a string representation of this {@code URL}.
     *
     * @return  a string representation of this object.
     */
    public String toExternalForm() {
        StringBuilder sb = new StringBuilder();
        sb.append(protocol).append(":");
        if (!host.isEmpty()) {
            sb.append("//").append(host);
            if (port >= 0) sb.append(":").append(port);
        }
        sb.append(file);
        if (ref != null) sb.append("#").append(ref);
        return sb.toString();
    }

    /**
     * Creates a string representation of this object.
     *
     * @return  a string representation of this object.
     */
    @Override
    public String toString() {
        return toExternalForm();
    }

    /**
     * Compares this URL for equality with another object.
     *
     * @param   obj   the URL to compare against.
     * @return  {@code true} if the objects are the same; {@code false}
     *          otherwise.
     */
    @Override
    public boolean equals(Object obj) {
        if (!(obj instanceof URL)) return false;
        URL u = (URL) obj;
        return toExternalForm().equals(u.toExternalForm());
    }

    /**
     * Creates an integer suitable for hash table indexing.
     *
     * @return  a hash code for this {@code URL}.
     */
    @Override
    public int hashCode() {
        return toExternalForm().hashCode();
    }
}
