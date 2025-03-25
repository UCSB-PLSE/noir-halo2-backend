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

template Test(n) {
    signal input in;
    signal input expected[n];

    component n2b = Num2Bits(n);
    n2b.in <== in;

    for (var i = 0; i<n; i++) {
        n2b.out[i] === expected[i];
    }
}

component main = Test(1000);