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
