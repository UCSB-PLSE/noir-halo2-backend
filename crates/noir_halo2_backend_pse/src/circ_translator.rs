use crate::r1cs_reader::{R1CS, ConstraintVec};
use ark_bn254::Bn254;
use ark_ec::pairing::Pairing;
use circ::ir::{opt::{opt, Opt}, term::{lin, Computations, Value::{self, Field}}};
use circ::front::{Mode, FrontEnd};
use circ::cfg::{CircOpt, cfg};
use std::path::PathBuf;
use circ::front::zsharp::{self, ZSharpFE};
use circ::target::r1cs::{R1cs as CircR1cs, Lc, ProverData};
use rug::integer::Order::{Lsf, Msf};
use ark_bn254::{Config as Bn254Config, Fr};
use ark_ff::{BigInt, BigInteger, BigInteger256, PrimeField, Zero};
use std::collections::{BTreeSet, HashMap};
use std::hash::BuildHasherDefault;
use acvm::acir::{
    circuit::{Circuit, Opcode, PublicInputs},
    native_types::{Expression, Witness, WitnessMap}, 
    FieldElement
};
use std::ops::Neg;

pub fn lc_to_constraint_vec(lc: &Lc) -> ConstraintVec<Bn254> {
    let mut vec: Vec<(usize, <Bn254 as Pairing>::ScalarField)> = Vec::new();
    // constant term (use wire 0)
    let constant_bytes_be = lc.constant.i().to_digits::<u8>(Msf);
    let constant_field = <Bn254 as Pairing>::ScalarField::from_be_bytes_mod_order(&constant_bytes_be);
    vec.push((0, constant_field));
    // linear terms
    for m in &lc.monomials {
        let (var, coeff) = m;
        
        let coeff_bytes_be = coeff.i().to_digits::<u8>(Msf);
        let coeff_field = <Bn254 as Pairing>::ScalarField::from_be_bytes_mod_order(&coeff_bytes_be);
        vec.push((var.clone() + 1, coeff_field));
    }
    vec
}

pub fn transform_circ_r1cs_to_r1cs(
    circ_r1cs: &CircR1cs<String>,
) -> R1CS<ark_bn254::Bn254> {
    let num_inputs = circ_r1cs.public_idxs.len();
    let num_aux = circ_r1cs.next_idx - num_inputs;
    let num_variables = circ_r1cs.next_idx;

    let num_constraints = circ_r1cs.constraints.len() as usize;
    let mut constraints = Vec::with_capacity(num_constraints);
    for constraint in &circ_r1cs.constraints {
        let (a, b, c) = constraint;
        constraints.push((
            lc_to_constraint_vec(a), 
            lc_to_constraint_vec(b), 
            lc_to_constraint_vec(c)
        ));
    }
    R1CS { 
        num_inputs: num_inputs, 
        num_aux: num_aux, 
        num_variables: num_variables, 
        constraints: constraints, 
        wire_mapping: Some(circ_r1cs.idxs_signals.keys().copied().collect()),
    }
}

pub fn get_circuit(r1cs: &R1CS<Bn254>, prover_data: &ProverData, witness: &mut WitnessMap) -> Circuit {
    let mut public_wits = Vec::new();
    let mut tmp_wit_index = prover_data.r1cs.next_idx as u32;
    for i in prover_data.r1cs.public_idxs.iter() {
        public_wits.push(Witness::from(*i as u32));
    }

    let opcodess: Vec<Opcode> = r1cs
        .constraints
        .clone()
        .into_iter()
        .map(|(a, b, c)| {
            let a_exprs = unfold_expression(&r1cs_term_to_expr(a), &mut tmp_wit_index, witness);
            let b_exprs = unfold_expression(&r1cs_term_to_expr(b), &mut tmp_wit_index, witness);
            let c_exprs = unfold_expression(&r1cs_term_to_expr(c), &mut tmp_wit_index, witness);
            
            let fst_a_expr = a_exprs.first().unwrap().clone();
            let fst_b_expr = b_exprs.first().unwrap().clone();
            let fst_c_expr =  c_exprs.first().unwrap().clone();

            let a_mul_b = (&fst_a_expr * &fst_b_expr).expect("`a` and `b` are both linear");
            
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
        }).flatten().collect::<Vec<Opcode>>();
    // println!("opcodes: {:?}", opcodes);
    Circuit { 
        current_witness_index: r1cs.num_variables as u32, 
        opcodes: opcodess, 
        public_parameters: PublicInputs(BTreeSet::from_iter(vec![])), 
        return_values: PublicInputs(BTreeSet::from_iter(vec![]))
    }
}

fn r1cs_term_to_expr(term: ConstraintVec<Bn254>) -> Expression {
    // println!("term: {:?}", term);
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
    
    // println!("linear_combinations: {:?}", linear_combinations);

    Expression { mul_terms: Vec::new(), linear_combinations: linear_combinations, q_c: q_c }
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
        // println!("expr: {:?}", expr);
        for (coff, witness) in expr.linear_combinations.iter() {
            // println!("val_expr: {:?}, coeff {:?}", val_expr, coff);
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

pub fn get_witness<H>(map: &HashMap<String, Value, BuildHasherDefault<H>>, prover_data: &ProverData) -> WitnessMap {
    let mut wit = WitnessMap::new();
    // println!("{:?}", prover_data.r1cs.signal_idxs);
    for (i, v) in map {
        // println!("{}: {}", i, v);
        match prover_data.r1cs.signal_idxs.get(i) {
            Some(idx) => {
                let w = Witness::new(idx.clone() as u32);
                match v {
                    Field(f) => {
                        let f_bytes = f.signed_int().to_digits::<u8>(Msf);
                        let f_field = FieldElement::from_be_bytes_reduce(&f_bytes);
                        
                        wit.insert(w, f_field);
                    }
                    _ => {
                        panic!("Unsupported value type: {:?}", v);
                    }
                }
            }
            None => {},
        }
    }
    wit
}
