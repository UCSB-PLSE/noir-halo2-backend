use std::collections::BTreeSet;

use super::r1cs_reader::R1CSFile;
use ark_ff::Zero;
use std::ops::Neg;

use acvm::{
    acir::{
        circuit::{Circuit, Opcode, PublicInputs},
        native_types::{Expression, Witness, WitnessMap},
    },
    FieldElement,
};

pub(crate) fn acir_circuit_from_r1cs_file(r1cs_file: R1CSFile<ark_bn254::Bn254>, tmp_wit_index: &mut u32, witness: &mut WitnessMap) -> Circuit {
    let num_public_outputs = r1cs_file.header.n_pub_out;
    let num_public_inputs = r1cs_file.header.n_pub_in;
    let num_private_inputs = r1cs_file.header.n_prv_in;
    let num_variables = r1cs_file.header.n_wires;

    // In a circom circuit, the inputs are arranged in the order:
    //
    // 1. Public outputs: 0..num_public_outputs
    // 2. Public inputs: num_public_outputs..+ num_public_inputs
    // 3. Private inputs: num_public_outputs + num_public_inputs..+ num_private_inputs
    let public_inputs_start = num_public_outputs;
    let private_inputs_start = public_inputs_start + num_public_inputs;

    let return_values: BTreeSet<Witness> = (0..public_inputs_start).map(Witness::from).collect();
    let public_parameters: BTreeSet<Witness> =
        (0..num_public_inputs).map(|i| Witness::from(public_inputs_start + i)).collect();
    let private_parameters: BTreeSet<Witness> =
        (0..num_private_inputs).map(|i| Witness::from(private_inputs_start + i)).collect();

    println!(
        "Public outputs: {}, Public inputs: {}, Private inputs: {}",
        num_public_outputs, num_public_inputs, num_private_inputs
    );

    let opcodes = r1cs_file
        .constraints
        .into_iter()
        .map(|(a, b, c)| {
            let a_exprs = unfold_expression(&r1cs_term_to_expr(a), tmp_wit_index, witness);
            let b_exprs = unfold_expression(&r1cs_term_to_expr(b), tmp_wit_index, witness);
            let c_exprs = unfold_expression(&r1cs_term_to_expr(c), tmp_wit_index, witness);
            
            let fst_a_expr = a_exprs.first().unwrap().clone();
            let fst_b_expr = b_exprs.first().unwrap().clone();
            let fst_c_expr =  c_exprs.first().unwrap().clone();

            let a_mul_b = (&fst_a_expr * &fst_b_expr).expect("`a` and `b` are both linear");
            // println!("c: {:?}", &c_exprs);
            println!("a*b - c: {:?}", &a_mul_b - &fst_c_expr);

            let mut opcodes = vec![Opcode::Arithmetic(&a_mul_b - &fst_c_expr)];
            for expr in a_exprs.into_iter().skip(1) {
                opcodes.push(Opcode::Arithmetic(expr));
            }
            for expr in b_exprs.into_iter().skip(1) {
                opcodes.push(Opcode::Arithmetic(expr));
            }
            for expr in c_exprs.into_iter().skip(1) {
                opcodes.push(Opcode::Arithmetic(expr));
            }
            opcodes
        })
        .flatten().collect();
    Circuit {
        current_witness_index: *tmp_wit_index,
        opcodes,
        // public_parameters: PublicInputs(public_parameters),
        public_parameters: PublicInputs(BTreeSet::new()),
        // return_values: PublicInputs(return_values),
        return_values: PublicInputs(BTreeSet::new()),
    }
}

fn r1cs_term_to_expr(term: Vec<(usize, ark_bn254::Fr)>) -> Expression {
    // constant term
    let q_c = term
        .iter()
        // in circom, wire 0 is always 1
        .filter(|(wire_index, _)| *wire_index == 0)
        // sum up all constant terms
        .fold(ark_bn254::Fr::zero(), |acc, (_, coeff)| acc + coeff);

    let q_c = match FieldElement::try_from_str(&q_c.to_string()) {
        Some(q_c) => q_c,
        None => FieldElement::zero(),
    };

    let linear_combinations = term
        .into_iter()
        .filter(|(wire_index, _)| *wire_index != 0)
        .map(|(wire_index, coefficient)| {
            (
                FieldElement::try_from_str(&coefficient.to_string()).unwrap(),
                // Subtract off 1 as witness 0 is not implicitly equal to 1 anymore and so can be
                // used.
                Witness(wire_index as u32 - 1),
            )
        })
        .collect();

    Expression { mul_terms: Vec::new(), linear_combinations, q_c }
}


fn unfold_expression(expr: &Expression, tmp_wit_index: &mut u32, witness_map: &mut WitnessMap) -> Vec<Expression> {
    let mut exprs = Vec::new();
    
    if expr.linear_combinations.len() <= 1 {
        exprs.push(expr.clone());
    } else {
        exprs.push(Expression {
            mul_terms: Vec::new(),
            linear_combinations: vec![(FieldElement::one(), Witness(tmp_wit_index.clone() as u32))],
            q_c: expr.q_c,
        });
        let mut val_expr: FieldElement = FieldElement::zero();
        for (coff, witness) in expr.linear_combinations.iter() {
            val_expr = val_expr + coff.clone() * witness_map.get(witness).unwrap_or(&FieldElement::zero()).clone();
        }
        witness_map.insert(Witness(*tmp_wit_index), val_expr.clone());
        
        for i in 0..expr.linear_combinations.len() {
            let (coeff, witness) = expr.linear_combinations[i].clone();
            if i < expr.linear_combinations.len() - 1 {
                let mut new_expr = Expression { 
                    mul_terms: Vec::new(), 
                    linear_combinations: vec![
                        (coeff, Witness(witness.0 as u32)),
                        (FieldElement::one(), Witness(*tmp_wit_index + 1 as u32)),
                        (FieldElement::one().neg(), Witness(tmp_wit_index.clone() as u32)),
                    ], 
                    q_c: FieldElement::zero(), 
                };
                exprs.push(new_expr);
                val_expr = val_expr - coeff.clone() * witness_map.get(&witness).unwrap_or(&FieldElement::zero()).clone();
                witness_map.insert(Witness(*tmp_wit_index + 1), val_expr.clone());

                *tmp_wit_index += 1;
            } else {
                let new_expr = Expression { 
                    mul_terms: Vec::new(), 
                    linear_combinations: vec![
                        (coeff, Witness(witness.0 as u32)),
                        (FieldElement::one().neg(), Witness(tmp_wit_index.clone() as u32)),
                    ], 
                    q_c: FieldElement::zero(), 
                };
                exprs.push(new_expr);
                *tmp_wit_index += 1;
            }
        }
    }

    exprs
}