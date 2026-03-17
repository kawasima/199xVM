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

/*
 * (C) Copyright Taligent, Inc. 1996, 1997 - All Rights Reserved
 * (C) Copyright IBM Corp. 1996 - 1998 - All Rights Reserved
 *
 *   The original version of this source code and documentation is copyrighted
 * and owned by Taligent, Inc., a wholly-owned subsidiary of IBM. These
 * materials are provided under terms of a License Agreement between Taligent
 * and Sun. This technology is protected by multiple US and International
 * patents. This notice and attribution to Taligent may not be removed.
 *   Taligent is a registered trademark of Taligent, Inc.
 *
 */

package java.text;

import java.io.Serializable;
import java.util.Locale;
import java.util.Objects;

/**
 * This class represents the set of symbols (such as the decimal separator,
 * the grouping separator, and so on) needed by {@code DecimalFormat}
 * to format numbers. {@code DecimalFormat} creates for itself an instance of
 * {@code DecimalFormatSymbols} from its locale data.  If you need to change any
 * of these symbols, you can get the {@code DecimalFormatSymbols} object from
 * your {@code DecimalFormat} and modify it.
 *
 * <p>If the locale contains "rg" (region override)
 * <a href="../util/Locale.html#def_locale_extension">Unicode extension</a>,
 * the symbols are overridden for the designated region.
 *
 * @see          java.util.Locale
 * @see          DecimalFormat
 * @author       Mark Davis
 * @author       Alan Liu
 * @since 1.1
 */

public class DecimalFormatSymbols implements Cloneable, Serializable {

    /**
     * Create a DecimalFormatSymbols object for the default
     * {@link java.util.Locale.Category#FORMAT FORMAT} locale.
     * This constructor can only construct instances for the locales
     * supported by the Java runtime environment, not for those
     * supported by installed
     * {@link java.text.spi.DecimalFormatSymbolsProvider DecimalFormatSymbolsProvider}
     * implementations. For full locale coverage, use the
     * {@link #getInstance(Locale) getInstance} method.
     * <p>This is equivalent to calling
     * {@link #DecimalFormatSymbols(Locale)
     *     DecimalFormatSymbols(Locale.getDefault(Locale.Category.FORMAT))}.
     * @see java.util.Locale#getDefault(java.util.Locale.Category)
     * @see java.util.Locale.Category#FORMAT
     */
    public DecimalFormatSymbols() {
        initialize( Locale.getDefault(Locale.Category.FORMAT) );
    }

    /**
     * Create a DecimalFormatSymbols object for the given locale.
     * This constructor can only construct instances for the locales
     * supported by the Java runtime environment, not for those
     * supported by installed
     * {@link java.text.spi.DecimalFormatSymbolsProvider DecimalFormatSymbolsProvider}
     * implementations. For full locale coverage, use the
     * {@link #getInstance(Locale) getInstance} method.
     * If the specified locale contains the {@link java.util.Locale#UNICODE_LOCALE_EXTENSION}
     * for the numbering system, the instance is initialized with the specified numbering
     * system if the JRE implementation supports it. For example,
     * <pre>
     * NumberFormat.getNumberInstance(Locale.forLanguageTag("th-TH-u-nu-thai"))
     * </pre>
     * This may return a {@code NumberFormat} instance with the Thai numbering system,
     * instead of the Latin numbering system.
     *
     * @param locale the desired locale
     * @throws    NullPointerException if {@code locale} is null
     */
    public DecimalFormatSymbols( Locale locale ) {
        initialize( locale );
    }

    /**
     * Returns an array of all locales for which the
     * {@code getInstance} methods of this class can return
     * localized instances.
     * The returned array represents the union of locales supported by the Java
     * runtime and by installed
     * {@link java.text.spi.DecimalFormatSymbolsProvider DecimalFormatSymbolsProvider}
     * implementations. At a minimum, the returned array must contain a
     * {@code Locale} instance equal to {@link Locale#ROOT Locale.ROOT} and
     * a {@code Locale} instance equal to {@link Locale#US Locale.US}.
     *
     * @return an array of locales for which localized
     *         {@code DecimalFormatSymbols} instances are available.
     * @since 1.6
     */
    public static Locale[] getAvailableLocales() {
        return new Locale[] { Locale.ROOT, Locale.US };
    }

    /**
     * Gets the {@code DecimalFormatSymbols} instance for the default
     * locale.  This method provides access to {@code DecimalFormatSymbols}
     * instances for locales supported by the Java runtime itself as well
     * as for those supported by installed
     * {@link java.text.spi.DecimalFormatSymbolsProvider
     * DecimalFormatSymbolsProvider} implementations.
     * <p>This is equivalent to calling
     * {@link #getInstance(Locale)
     *     getInstance(Locale.getDefault(Locale.Category.FORMAT))}.
     * @see java.util.Locale#getDefault(java.util.Locale.Category)
     * @see java.util.Locale.Category#FORMAT
     * @return a {@code DecimalFormatSymbols} instance.
     * @since 1.6
     */
    public static final DecimalFormatSymbols getInstance() {
        return getInstance(Locale.getDefault(Locale.Category.FORMAT));
    }

    /**
     * Gets the {@code DecimalFormatSymbols} instance for the specified
     * locale.  This method provides access to {@code DecimalFormatSymbols}
     * instances for locales supported by the Java runtime itself as well
     * as for those supported by installed
     * {@link java.text.spi.DecimalFormatSymbolsProvider
     * DecimalFormatSymbolsProvider} implementations.
     * If the specified locale contains the {@link java.util.Locale#UNICODE_LOCALE_EXTENSION}
     * for the numbering system, the instance is initialized with the specified numbering
     * system if the JRE implementation supports it. For example,
     * <pre>
     * NumberFormat.getNumberInstance(Locale.forLanguageTag("th-TH-u-nu-thai"))
     * </pre>
     * This may return a {@code NumberFormat} instance with the Thai numbering system,
     * instead of the Latin numbering system.
     *
     * @param locale the desired locale.
     * @return a {@code DecimalFormatSymbols} instance.
     * @throws    NullPointerException if {@code locale} is null
     * @since 1.6
     */
    public static final DecimalFormatSymbols getInstance(Locale locale) {
        // 199xVM shim: bypass LocaleProviderAdapter, construct directly.
        return new DecimalFormatSymbols(locale);
    }

    /**
     * {@return locale used to create this instance}
     *
     * @since 19
     */
    public Locale getLocale() {
        return locale;
    }

    /**
     * Gets the character used for zero. Different for Arabic, etc.
     *
     * @return the character used for zero
     */
    public char getZeroDigit() {
        return zeroDigit;
    }

    /**
     * Sets the character used for zero. Different for Arabic, etc.
     *
     * @param zeroDigit the character used for zero
     */
    public void setZeroDigit(char zeroDigit) {
        hashCode = 0;
        this.zeroDigit = zeroDigit;
    }

    /**
     * Gets the character used for grouping separator. Different for French, etc.
     *
     * @return the grouping separator
     */
    public char getGroupingSeparator() {
        return groupingSeparator;
    }

    /**
     * Sets the character used for grouping separator. Different for French, etc.
     *
     * @param groupingSeparator the grouping separator
     */
    public void setGroupingSeparator(char groupingSeparator) {
        hashCode = 0;
        this.groupingSeparator = groupingSeparator;
    }

    /**
     * Gets the character used for decimal sign. Different for French, etc.
     *
     * @return the character used for decimal sign
     */
    public char getDecimalSeparator() {
        return decimalSeparator;
    }

    /**
     * Sets the character used for decimal sign. Different for French, etc.
     *
     * @param decimalSeparator the character used for decimal sign
     */
    public void setDecimalSeparator(char decimalSeparator) {
        hashCode = 0;
        this.decimalSeparator = decimalSeparator;
    }

    /**
     * Gets the character used for per mille sign. Different for Arabic, etc.
     *
     * @return the character used for per mille sign
     */
    public char getPerMill() {
        return perMill;
    }

    /**
     * Sets the character used for per mille sign. Different for Arabic, etc.
     *
     * @param perMill the character used for per mille sign
     */
    public void setPerMill(char perMill) {
        hashCode = 0;
        this.perMill = perMill;
        this.perMillText = Character.toString(perMill);
    }

    /**
     * Gets the character used for percent sign. Different for Arabic, etc.
     *
     * @return the character used for percent sign
     */
    public char getPercent() {
        return percent;
    }

    /**
     * Sets the character used for percent sign. Different for Arabic, etc.
     *
     * @param percent the character used for percent sign
     */
    public void setPercent(char percent) {
        hashCode = 0;
        this.percent = percent;
        this.percentText = Character.toString(percent);
    }

    /**
     * Gets the character used for a digit in a pattern.
     *
     * @return the character used for a digit in a pattern
     */
    public char getDigit() {
        return digit;
    }

    /**
     * Sets the character used for a digit in a pattern.
     *
     * @param digit the character used for a digit in a pattern
     */
    public void setDigit(char digit) {
        hashCode = 0;
        this.digit = digit;
    }

    /**
     * Gets the character used to separate positive and negative subpatterns
     * in a pattern.
     *
     * @return the pattern separator
     */
    public char getPatternSeparator() {
        return patternSeparator;
    }

    /**
     * Sets the character used to separate positive and negative subpatterns
     * in a pattern.
     *
     * @param patternSeparator the pattern separator
     */
    public void setPatternSeparator(char patternSeparator) {
        hashCode = 0;
        this.patternSeparator = patternSeparator;
    }

    /**
     * Gets the string used to represent infinity. Almost always left
     * unchanged.
     *
     * @return the string representing infinity
     */
    public String getInfinity() {
        return infinity;
    }

    /**
     * Sets the string used to represent infinity. Almost always left
     * unchanged.
     *
     * @param infinity the string representing infinity
     * @throws NullPointerException if {@code infinity} is {@code null}
     */
    public void setInfinity(String infinity) {
        this.infinity = Objects.requireNonNull(infinity);
        hashCode = 0;
    }

    /**
     * Gets the string used to represent "not a number". Almost always left
     * unchanged.
     *
     * @return the string representing "not a number"
     */
    public String getNaN() {
        return NaN;
    }

    /**
     * Sets the string used to represent "not a number". Almost always left
     * unchanged.
     *
     * @param NaN the string representing "not a number"
     * @throws NullPointerException if {@code NaN} is {@code null}
     */
    public void setNaN(String NaN) {
        this.NaN = Objects.requireNonNull(NaN);
        hashCode = 0;
    }

    /**
     * Gets the character used to represent minus sign. If no explicit
     * negative format is specified, one is formed by prefixing
     * minusSign to the positive format.
     *
     * @return the character representing minus sign
     */
    public char getMinusSign() {
        return minusSign;
    }

    /**
     * Sets the character used to represent minus sign. If no explicit
     * negative format is specified, one is formed by prefixing
     * minusSign to the positive format.
     *
     * @param minusSign the character representing minus sign
     */
    public void setMinusSign(char minusSign) {
        hashCode = 0;
        this.minusSign = minusSign;
        this.minusSignText = Character.toString(minusSign);
    }

    /**
     * Returns the currency symbol for the currency of these
     * DecimalFormatSymbols in their locale.
     *
     * @return the currency symbol
     * @since 1.2
     */
    public String getCurrencySymbol()
    {
        initializeCurrency(locale);
        return currencySymbol;
    }

    /**
     * Sets the currency symbol for the currency of this
     * {@code DecimalFormatSymbols} in their locale. Unlike {@link
     * #setInternationalCurrencySymbol(String)}, this method does not update
     * the currency attribute nor the international currency symbol attribute.
     *
     * @param currency the currency symbol
     * @throws NullPointerException if {@code currency} is {@code null}
     * @since 1.2
     */
    public void setCurrencySymbol(String currency)
    {
        Objects.requireNonNull(currency);
        initializeCurrency(locale);
        hashCode = 0;
        currencySymbol = currency;
    }

    /**
     * Returns the ISO 4217 currency code of the currency of these
     * DecimalFormatSymbols.
     *
     * @return the currency code
     * @since 1.2
     */
    public String getInternationalCurrencySymbol()
    {
        initializeCurrency(locale);
        return intlCurrencySymbol;
    }

    /**
     * Sets the ISO 4217 currency code of the currency of these
     * DecimalFormatSymbols.
     * If the currency code is valid (as defined by
     * {@link java.util.Currency#getInstance(java.lang.String) Currency.getInstance}),
     * this also sets the currency attribute to the corresponding Currency
     * instance and the currency symbol attribute to the currency's symbol
     * in the DecimalFormatSymbols' locale. If the currency code is not valid,
     * then the currency attribute and the currency symbol attribute are not modified.
     *
     * @param currencyCode the currency code
     * @throws NullPointerException if {@code currencyCode} is {@code null}
     * @see #setCurrency
     * @see #setCurrencySymbol
     * @since 1.2
     */
    public void setInternationalCurrencySymbol(String currencyCode) {
        Objects.requireNonNull(currencyCode);
        initializeCurrency(locale);
        hashCode = 0;
        intlCurrencySymbol = currencyCode;
    }

    /**
     * {@return the {@code Currency} of this {@code DecimalFormatSymbols}}
     * @since 1.4
     */
    public Object getCurrency() {
        initializeCurrency(locale);
        return null;
    }

    /**
     * Returns the monetary decimal separator.
     *
     * @return the monetary decimal separator
     * @since 1.2
     */
    public char getMonetaryDecimalSeparator()
    {
        return monetarySeparator;
    }

    /**
     * Sets the monetary decimal separator.
     *
     * @param sep the monetary decimal separator
     * @since 1.2
     */
    public void setMonetaryDecimalSeparator(char sep)
    {
        hashCode = 0;
        monetarySeparator = sep;
    }

    /**
     * Returns the string used to separate the mantissa from the exponent.
     * Examples: "x10^" for 1.23x10^4, "E" for 1.23E4.
     *
     * @return the exponent separator string
     * @see #setExponentSeparator(java.lang.String)
     * @since 1.6
     */
    public String getExponentSeparator()
    {
        return exponentialSeparator;
    }

    /**
     * Sets the string used to separate the mantissa from the exponent.
     * Examples: "x10^" for 1.23x10^4, "E" for 1.23E4.
     *
     * @param exp the exponent separator string
     * @throws    NullPointerException if {@code exp} is null
     * @see #getExponentSeparator()
     * @since 1.6
     */
    public void setExponentSeparator(String exp)
    {
        Objects.requireNonNull(exp);
        hashCode = 0;
        exponentialSeparator = exp;
    }

    /**
     * Gets the character used for grouping separator for currencies.
     * May be different from {@code grouping separator} in some locales,
     * e.g, German in Austria.
     *
     * @return the monetary grouping separator
     * @since 15
     */
    public char getMonetaryGroupingSeparator() {
        return monetaryGroupingSeparator;
    }

    /**
     * Sets the character used for grouping separator for currencies.
     * Invocation of this method will not affect the normal
     * {@code grouping separator}.
     *
     * @param monetaryGroupingSeparator the monetary grouping separator
     * @see #setGroupingSeparator(char)
     * @since 15
     */
    public void setMonetaryGroupingSeparator(char monetaryGroupingSeparator)
    {
        hashCode = 0;
        this.monetaryGroupingSeparator = monetaryGroupingSeparator;
    }

    //------------------------------------------------------------
    // BEGIN   Package Private methods ... to be made public later
    //------------------------------------------------------------

    /**
     * Returns the character used to separate the mantissa from the exponent.
     */
    char getExponentialSymbol()
    {
        return exponential;
    }

    /**
     * Sets the character used to separate the mantissa from the exponent.
     */
    void setExponentialSymbol(char exp)
    {
        exponential = exp;
    }

    /**
     * Gets the string used for per mille sign. Different for Arabic, etc.
     *
     * @return the string used for per mille sign
     * @since 13
     */
    String getPerMillText() {
        return perMillText;
    }

    /**
     * Sets the string used for per mille sign. Different for Arabic, etc.
     *
     * Setting the {@code perMillText} affects the return value of
     * {@link #getPerMill()}, in which the first non-format character of
     * {@code perMillText} is returned.
     *
     * @param perMillText the string used for per mille sign
     * @throws NullPointerException if {@code perMillText} is null
     * @throws IllegalArgumentException if {@code perMillText} is an empty string
     * @see #getPerMill()
     * @see #getPerMillText()
     * @since 13
     */
    void setPerMillText(String perMillText) {
        Objects.requireNonNull(perMillText);
        if (perMillText.isEmpty()) {
            throw new IllegalArgumentException("Empty argument string");
        }

        hashCode = 0;
        this.perMillText = perMillText;
        this.perMill = findNonFormatChar(perMillText, '\u2030');
    }

    /**
     * Gets the string used for percent sign. Different for Arabic, etc.
     *
     * @return the string used for percent sign
     * @since 13
     */
    String getPercentText() {
        return percentText;
    }

    /**
     * Sets the string used for percent sign. Different for Arabic, etc.
     *
     * Setting the {@code percentText} affects the return value of
     * {@link #getPercent()}, in which the first non-format character of
     * {@code percentText} is returned.
     *
     * @param percentText the string used for percent sign
     * @throws NullPointerException if {@code percentText} is null
     * @throws IllegalArgumentException if {@code percentText} is an empty string
     * @see #getPercent()
     * @see #getPercentText()
     * @since 13
     */
    void setPercentText(String percentText) {
        Objects.requireNonNull(percentText);
        if (percentText.isEmpty()) {
            throw new IllegalArgumentException("Empty argument string");
        }

        hashCode = 0;
        this.percentText = percentText;
        this.percent = findNonFormatChar(percentText, '%');
    }

    /**
     * Gets the string used to represent minus sign. If no explicit
     * negative format is specified, one is formed by prefixing
     * minusSignText to the positive format.
     *
     * @return the string representing minus sign
     * @since 13
     */
    String getMinusSignText() {
        return minusSignText;
    }

    /**
     * Sets the string used to represent minus sign. If no explicit
     * negative format is specified, one is formed by prefixing
     * minusSignText to the positive format.
     *
     * Setting the {@code minusSignText} affects the return value of
     * {@link #getMinusSign()}, in which the first non-format character of
     * {@code minusSignText} is returned.
     *
     * @param minusSignText the character representing minus sign
     * @throws NullPointerException if {@code minusSignText} is null
     * @throws IllegalArgumentException if {@code minusSignText} is an
     *  empty string
     * @see #getMinusSign()
     * @see #getMinusSignText()
     * @since 13
     */
    void setMinusSignText(String minusSignText) {
        Objects.requireNonNull(minusSignText);
        if (minusSignText.isEmpty()) {
            throw new IllegalArgumentException("Empty argument string");
        }

        hashCode = 0;
        this.minusSignText = minusSignText;
        this.minusSign = findNonFormatChar(minusSignText, '-');
    }

    //------------------------------------------------------------
    // END     Package Private methods ... to be made public later
    //------------------------------------------------------------

    /**
     * Standard override.
     */
    @Override
    public Object clone() {
        try {
            return (DecimalFormatSymbols)super.clone();
            // other fields are bit-copied
        } catch (CloneNotSupportedException e) {
            throw new InternalError(e);
        }
    }

    /**
     * Compares the specified object with this {@code DecimalFormatSymbols} for equality.
     * Returns true if the object is also a {@code DecimalFormatSymbols} and the two
     * {@code DecimalFormatSymbols} objects represent the same set of symbols.
     *
     * @implSpec This method performs an equality check with a notion of class
     * identity based on {@code getClass()}, rather than {@code instanceof}.
     * Therefore, in the equals methods in subclasses, no instance of this class
     * should compare as equal to an instance of a subclass.
     * @param  obj object to be compared for equality
     * @return {@code true} if the specified object is equal to this {@code DecimalFormatSymbols}
     * @see Object#equals(Object)
     */
    @Override
    public boolean equals(Object obj) {
        if (this == obj) return true;
        if (obj == null || getClass() != obj.getClass()) return false;
        DecimalFormatSymbols other = (DecimalFormatSymbols) obj;
        return (zeroDigit == other.zeroDigit &&
            groupingSeparator == other.groupingSeparator &&
            decimalSeparator == other.decimalSeparator &&
            percent == other.percent &&
            percentText.equals(other.percentText) &&
            perMill == other.perMill &&
            perMillText.equals(other.perMillText) &&
            digit == other.digit &&
            minusSign == other.minusSign &&
            minusSignText.equals(other.minusSignText) &&
            patternSeparator == other.patternSeparator &&
            infinity.equals(other.infinity) &&
            NaN.equals(other.NaN) &&
            getCurrencySymbol().equals(other.getCurrencySymbol()) &&
            intlCurrencySymbol.equals(other.intlCurrencySymbol) &&
            monetarySeparator == other.monetarySeparator &&
            monetaryGroupingSeparator == other.monetaryGroupingSeparator &&
            exponentialSeparator.equals(other.exponentialSeparator) &&
            locale.equals(other.locale));
    }

    /**
     * {@return the hash code for this {@code DecimalFormatSymbols}}
     *
     * @implSpec Non-transient instance fields of this class are used to calculate
     * a hash code value which adheres to the contract defined in {@link Objects#hashCode}.
     * @see Object#hashCode()
     */
    @Override
    public int hashCode() {
        if (hashCode == 0) {
            hashCode = Objects.hash(
                zeroDigit,
                groupingSeparator,
                decimalSeparator,
                percent,
                percentText,
                perMill,
                perMillText,
                digit,
                minusSign,
                minusSignText,
                patternSeparator,
                infinity,
                NaN,
                getCurrencySymbol(),
                intlCurrencySymbol,
                monetarySeparator,
                monetaryGroupingSeparator,
                exponentialSeparator,
                locale);
        }
        return hashCode;
    }

    /**
     * Initializes the symbols from the FormatData resource bundle.
     *
     * 199xVM shim: instead of loading from LocaleProviderAdapter / resource
     * bundles, hardcode the en_US / ROOT locale symbols.
     */
    private void initialize( Locale locale ) {
        this.locale = locale;

        // en_US / ROOT defaults (matches CLDR data for "latn" numbering system)
        decimalSeparator = '.';
        groupingSeparator = ',';
        patternSeparator = ';';
        percentText = "%";
        percent = '%';
        zeroDigit = '0';
        digit = '#';
        minusSignText = "-";
        minusSign = '-';
        exponential = 'E';
        exponentialSeparator = "E";
        perMillText = "\u2030";
        perMill = '\u2030';
        infinity  = "\u221E";
        NaN = "NaN";

        monetarySeparator = decimalSeparator;
        monetaryGroupingSeparator = groupingSeparator;

        // Currency defaults
        intlCurrencySymbol = "USD";
        currencySymbol = "$";
    }

    /**
     * Obtains non-format single character from String
     */
    private char findNonFormatChar(String src, char defChar) {
        for (int i = 0; i < src.length(); i++) {
            char c = src.charAt(i);
            if (Character.getType(c) != Character.FORMAT) {
                return c;
            }
        }
        return defChar;
    }

    /**
     * Lazy initialization for currency related fields.
     *
     * 199xVM shim: Currency class is not available; use hardcoded defaults.
     */
    private void initializeCurrency(Locale locale) {
        if (currencyInitialized) {
            return;
        }
        // defaults already set in initialize()
        currencyInitialized = true;
    }

    /**
     * Character used for zero.
     *
     * @serial
     * @see #getZeroDigit
     */
    private  char    zeroDigit;

    /**
     * Character used for grouping separator.
     *
     * @serial
     * @see #getGroupingSeparator
     */
    private  char    groupingSeparator;

    /**
     * Character used for decimal sign.
     *
     * @serial
     * @see #getDecimalSeparator
     */
    private  char    decimalSeparator;

    /**
     * Character used for per mille sign.
     *
     * @serial
     * @see #getPerMill
     */
    private  char    perMill;

    /**
     * Character used for percent sign.
     * @serial
     * @see #getPercent
     */
    private  char    percent;

    /**
     * Character used for a digit in a pattern.
     *
     * @serial
     * @see #getDigit
     */
    private  char    digit;

    /**
     * Character used to separate positive and negative subpatterns
     * in a pattern.
     *
     * @serial
     * @see #getPatternSeparator
     */
    private  char    patternSeparator;

    /**
     * String used to represent infinity.
     * @serial
     * @see #getInfinity
     */
    private  String  infinity;

    /**
     * String used to represent "not a number".
     * @serial
     * @see #getNaN
     */
    private  String  NaN;

    /**
     * Character used to represent minus sign.
     * @serial
     * @see #getMinusSign
     */
    private  char    minusSign;

    /**
     * String denoting the local currency, e.g. "$".
     * @serial
     * @see #getCurrencySymbol
     */
    private  String  currencySymbol;

    /**
     * ISO 4217 currency code denoting the local currency, e.g. "USD".
     * @serial
     * @see #getInternationalCurrencySymbol
     */
    private  String  intlCurrencySymbol;

    /**
     * The decimal separator used when formatting currency values.
     * @serial
     * @since  1.1.6
     * @see #getMonetaryDecimalSeparator
     */
    private  char    monetarySeparator; // Field new in JDK 1.1.6

    /**
     * The character used to distinguish the exponent in a number formatted
     * in exponential notation, e.g. 'E' for a number such as "1.23E45".
     * <p>
     * Note that the public API provides no way to set this field,
     * even though it is supported by the implementation and the stream format.
     * The intent is that this will be added to the API in the future.
     *
     * @serial
     * @since  1.1.6
     */
    private  char    exponential;       // Field new in JDK 1.1.6

    /**
     * The string used to separate the mantissa from the exponent.
     * Examples: "x10^" for 1.23x10^4, "E" for 1.23E4.
     * <p>
     * If both {@code exponential} and {@code exponentialSeparator}
     * exist, this {@code exponentialSeparator} has the precedence.
     *
     * @serial
     * @since 1.6
     */
    private  String    exponentialSeparator;       // Field new in JDK 1.6

    /**
     * The locale of these currency format symbols.
     *
     * @serial
     * @since 1.4
     */
    private Locale locale;

    /**
     * String representation of per mille sign, which may include
     * formatting characters, such as BiDi control characters.
     * The first non-format character of this string is the same as
     * {@code perMill}.
     *
     * @serial
     * @since 13
     */
    private  String perMillText;

    /**
     * String representation of percent sign, which may include
     * formatting characters, such as BiDi control characters.
     * The first non-format character of this string is the same as
     * {@code percent}.
     *
     * @serial
     * @since 13
     */
    private  String percentText;

    /**
     * String representation of minus sign, which may include
     * formatting characters, such as BiDi control characters.
     * The first non-format character of this string is the same as
     * {@code minusSign}.
     *
     * @serial
     * @since 13
     */
    private  String minusSignText;

    /**
     * The grouping separator used when formatting currency values.
     *
     * @serial
     * @since 15
     */
    private  char    monetaryGroupingSeparator;

    private transient volatile boolean currencyInitialized;

    /**
     * Cached hash code.
     */
    private transient volatile int hashCode;

    static final long serialVersionUID = 5772796243397350300L;
}
