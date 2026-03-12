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

package java.util;

import java.io.Serializable;

public final class Locale implements Cloneable, Serializable {
    public enum Category { DISPLAY, FORMAT }
    public static final Locale ROOT = new Locale("", "", "");
    public static final Locale US = new Locale("en", "US");
    public static final Locale ENGLISH = new Locale("en", "");

    private static Locale defaultLocale = US;

    private final String language;
    private final String country;
    private final String variant;

    public Locale(String language, String country) {
        this(language, country, "");
    }

    public Locale(String language, String country, String variant) {
        this.language = language == null ? "" : language;
        this.country = country == null ? "" : country;
        this.variant = variant == null ? "" : variant;
    }

    public static Locale getDefault() {
        return defaultLocale;
    }

    public static Locale getDefault(Category category) {
        return getDefault();
    }

    public static void setDefault(Locale newLocale) {
        defaultLocale = (newLocale == null) ? US : newLocale;
    }

    public static Locale forLanguageTag(String languageTag) {
        if (languageTag == null || languageTag.length() == 0) {
            return ROOT;
        }
        int dash = languageTag.indexOf('-');
        if (dash < 0) {
            return new Locale(languageTag, "");
        }
        String lang = languageTag.substring(0, dash);
        String country = languageTag.substring(dash + 1);
        return new Locale(lang, country);
    }

    public String getLanguage() {
        return language;
    }

    public String getCountry() {
        return country;
    }

    public String getVariant() {
        return variant;
    }

    public String toLanguageTag() {
        if (country.length() == 0) {
            return language;
        }
        return language + "-" + country;
    }

    public String getUnicodeLocaleType(String key) {
        return null;
    }

    @Override
    public String toString() {
        if (country.length() == 0) return language;
        return language + "_" + country;
    }

    @Override
    public int hashCode() {
        int h = language.hashCode();
        h = 31 * h + country.hashCode();
        h = 31 * h + variant.hashCode();
        return h;
    }

    @Override
    public boolean equals(Object obj) {
        if (this == obj) return true;
        if (!(obj instanceof Locale)) return false;
        Locale other = (Locale) obj;
        return language.equals(other.language)
            && country.equals(other.country)
            && variant.equals(other.variant);
    }
}
