pragma circom 2.0.0;

template Num2Bits(n) {
    signal input in;
    signal output out[n];
    var lc1=0;

    var e2=1;
    for (var i = 0; i<n; i++) {
        out[i] <-- (in >> i) & 1;
        out[i] * (out[i] -1 ) === 0;
        lc1 += out[i] * e2;
        e2 = e2+e2;
    }

    lc1 === in;
}

template ModSumFour(n) {
    assert(n + 2 <= 253);
    signal input a;
    signal input b;
    signal input c;
    signal input d;
    signal output sum;
    signal output carry;

    component n2b = Num2Bits(n + 2);
    n2b.in <== a + b + c + d;
    carry <== n2b.out[n] + 2 * n2b.out[n + 1];
    sum <== a + b + c + d - carry * (1 << n);
}
component main = ModSumFour(251);
