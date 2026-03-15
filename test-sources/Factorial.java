long fact(int n) {
    long result = 1;
    for (int i = 2; i <= n; i++) {
        result = result * i;
    }
    return result;
}

String run() {
    return "10!=" + fact(10) + " 15!=" + fact(15);
}
