global size: Field = TBD;
global range : Field = 9;

fn check_cards(a: [Field; size]) {
    let mut s : Field = 0;
    for n in 1..range+1 {
        let mut p : Field = 1;
  	    for j in 0..size {
          p = p * (n - a[j]);
        };
        s = s + p;
    };

    let mut p : Field = 1;
    for j in 0..size {
        p = p * a[j];
    };
    assert (p != 0);
    assert (s == 0);
}

fn main(a: [Field; size])  {
    check_cards(a);
}