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

import java.io.Serializable;

/**
 * This class represents an Internet Protocol (IP) address.
 *
 * <p>This shim provides only loopback/localhost functionality.
 * Network I/O (hostname resolution) is not supported in 199xVM.
 *
 * @author  Chris Warth
 * @see     java.net.InetAddress#getByName(java.lang.String)
 * @since   1.0
 */
public class InetAddress implements Serializable {

    @java.io.Serial
    private static final long serialVersionUID = 3286316764910316507L;

    private final String hostName;
    private final byte[] address;

    InetAddress(String hostName, byte[] address) {
        this.hostName = hostName;
        this.address = address;
    }

    /**
     * Gets the host name for this IP address.
     *
     * @return  the host name for this IP address, or the textual
     *          representation if no hostname is available.
     */
    public String getHostName() {
        return hostName;
    }

    /**
     * Returns the IP address string in textual presentation form.
     *
     * @return  the raw IP address in a string format.
     */
    public String getHostAddress() {
        if (address == null || address.length == 0) return "";
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < address.length; i++) {
            if (i > 0) sb.append('.');
            sb.append(address[i] & 0xFF);
        }
        return sb.toString();
    }

    /**
     * Returns the raw IP address of this {@code InetAddress} object.
     *
     * @return  the raw IP address of this object.
     */
    public byte[] getAddress() {
        return address.clone();
    }

    /**
     * Returns {@code true} if the InetAddress is a loopback address.
     *
     * @return a {@code boolean} indicating if the InetAddress is
     *         a loopback address; or false otherwise.
     * @since 1.4
     */
    public boolean isLoopbackAddress() {
        return address != null && address.length == 4
            && address[0] == 127;
    }

    /**
     * Returns the loopback address.
     *
     * @return  the InetAddress loopback instance.
     * @since 1.7
     */
    public static InetAddress getLoopbackAddress() {
        return new InetAddress("localhost", new byte[]{127, 0, 0, 1});
    }

    /**
     * Returns the address of the local host.
     *
     * <p>This shim always returns the loopback address.
     *
     * @return  the address of the local host.
     * @throws  UnknownHostException  if the local host name could not
     *          be resolved into an address.
     */
    public static InetAddress getLocalHost() throws UnknownHostException {
        return getLoopbackAddress();
    }

    /**
     * Determines the IP address of a host, given the host's name.
     *
     * <p>Network access is not supported in 199xVM; this method always
     * throws {@code UnknownHostException} unless the host name refers to
     * the loopback address.
     *
     * @param   host  the specified host, or {@code null} for the loopback address.
     * @return  an IP address for the given host name.
     * @throws  UnknownHostException  if no IP address for the host could be found.
     */
    public static InetAddress getByName(String host) throws UnknownHostException {
        if (host == null || host.equals("localhost") || host.equals("127.0.0.1")) {
            return getLoopbackAddress();
        }
        throw new UnknownHostException("Network access not supported in 199xVM: " + host);
    }

    /**
     * Given the name of a host, returns an array of its IP addresses.
     *
     * <p>Network access is not supported in 199xVM; this method always
     * throws {@code UnknownHostException} unless the host name refers to
     * the loopback address.
     *
     * @param   host  the name of the host, or {@code null} for the loopback address.
     * @return  an array of all the IP addresses for a given host name.
     * @throws  UnknownHostException  if no IP address for the host could be found.
     */
    public static InetAddress[] getAllByName(String host) throws UnknownHostException {
        return new InetAddress[]{ getByName(host) };
    }

    /**
     * Returns a string representation of this IP address.
     *
     * @return  a string representation of this IP address.
     */
    @Override
    public String toString() {
        return ((hostName != null) ? hostName : "") + "/" + getHostAddress();
    }

    /**
     * Compares this object against the specified object.
     *
     * @param   obj   the object to compare against.
     * @return  {@code true} if the objects are the same;
     *          {@code false} otherwise.
     */
    @Override
    public boolean equals(Object obj) {
        if (this == obj) return true;
        if (!(obj instanceof InetAddress)) return false;
        InetAddress other = (InetAddress) obj;
        if (address == null || other.address == null) return false;
        if (address.length != other.address.length) return false;
        for (int i = 0; i < address.length; i++) {
            if (address[i] != other.address[i]) return false;
        }
        return true;
    }

    /**
     * Returns a hashcode for this IP address.
     *
     * @return  a hash code value for this IP address.
     */
    @Override
    public int hashCode() {
        if (address == null) return 0;
        int result = 0;
        for (byte b : address) result = 31 * result + (b & 0xFF);
        return result;
    }
}
