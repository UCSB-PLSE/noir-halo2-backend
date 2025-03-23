use std::collections::BTreeSet;

use super::r1cs_reader::R1CSFile;
use ark_ff::Zero;

use acvm::{
    acir::{
        circuit::{Circuit, Opcode, PublicInputs},
        native_types::{Expression, Witness},
    },
    FieldElement,
};

pub(crate) fn acir_circuit_from_r1cs_file(r1cs_file: R1CSFile<ark_bn254::Bn254>) -> Circuit {
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
            let a_expr = r1cs_term_to_expr(a);
            let b_expr = r1cs_term_to_expr(b);
            let c_expr = r1cs_term_to_expr(c);

            let a_mul_b = (&a_expr * &b_expr).expect("`a` and `b` are both linear");
            println!("a*b - c: {:?}", &a_mul_b - &c_expr);
            Opcode::Arithmetic(&a_mul_b - &c_expr)
        })
        .collect();

    Circuit {
        current_witness_index: num_variables,
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
