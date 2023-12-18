global n: Field = 256;

fn main(coord: Field, ship_coords: [Field; n], c: Field)  {
    let mut collisions = 0;
    for i in 0..n {
        if ship_coords[i] == coord {
            collisions = collisions + 1;
        }
        // collisions = collisions + () as Field;
    };
    assert (collisions == c);
}

