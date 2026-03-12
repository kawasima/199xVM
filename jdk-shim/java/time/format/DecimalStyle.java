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

package java.time.format;

public final class DecimalStyle {
    public static final DecimalStyle STANDARD = new DecimalStyle('0', '+', '-', '.');

    private final char zeroDigit;
    private final char positiveSign;
    private final char negativeSign;
    private final char decimalSeparator;

    public DecimalStyle(char zeroDigit, char positiveSign, char negativeSign, char decimalSeparator) {
        this.zeroDigit = zeroDigit;
        this.positiveSign = positiveSign;
        this.negativeSign = negativeSign;
        this.decimalSeparator = decimalSeparator;
    }

    public char getZeroDigit() { return zeroDigit; }
    public char getPositiveSign() { return positiveSign; }
    public char getNegativeSign() { return negativeSign; }
    public char getDecimalSeparator() { return decimalSeparator; }

    public static DecimalStyle of(java.util.Locale locale) { return STANDARD; }

    public int convertToDigit(char ch) {
        int val = ch - zeroDigit;
        return (val >= 0 && val <= 9) ? val : -1;
    }

    public String convertNumberToI18N(String numericText) { return numericText; }

    @Override
    public String toString() {
        return "DecimalStyle[" + zeroDigit + positiveSign + negativeSign + decimalSeparator + "]";
    }
}
