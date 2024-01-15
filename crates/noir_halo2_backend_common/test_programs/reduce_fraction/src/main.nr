struct Fraction {
    num: u32,
    den: u32,
}

// this method will only work till numerator and denominator values are under 100
// this has been set for efficiency reasons, and will be modified once the Noir team
// can implement dynamic limit for loops
fn main(f: Fraction) -> pub Fraction {
    let mut a = f.num;
    let mut b = f.den;
    let mut min = 0;
    let mut j = 1;
    let mut gcd = 1;
    if a > b {
        min = b;
    }
    else {
        min = a;
    }
    
    for i in 2..100{
        j = i as u32;
        if (j <= min){
            if (a%j == 0) & (b%j == 0){
                gcd = j;
            }
        }
    }
    
    
    let fr = Fraction {
        num: f.num/gcd,
        den: f.den/gcd,
    };
    fr
}