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

template Bits2Num(n) {
    signal input in[n];
    signal output out;
    var lc1=0;

    var e2 = 1;
    for (var i = 0; i<n; i++) {
        lc1 += in[i] * e2;
        e2 = e2 + e2;
    }

    lc1 ==> out;
}

template ModProd(n) {
    assert(n <= 126);
    signal input a;
    signal input b;
    signal output prod;
    signal output carry;

    component n2b = Num2Bits(2 * n);
    n2b.in <== a * b;

    component b2n1 = Bits2Num(n);
    component b2n2 = Bits2Num(n);
    var i;
    for (i = 0; i < n; i++) {
        b2n1.in[i] <== n2b.out[i];
        b2n2.in[i] <== n2b.out[i + n];
    }
    prod <== b2n1.out;
    carry <== b2n2.out;
}

component main = ModProd(126);
