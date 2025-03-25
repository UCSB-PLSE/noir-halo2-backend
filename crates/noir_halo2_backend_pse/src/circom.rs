use super::{
    circom_circuit, circom_witness,
    r1cs_reader::{IoResult, R1CSFile},
};
use acvm::acir::{circuit::Circuit, native_types::WitnessMap};
use core::num;
use std::{
    collections::BTreeMap,
    env::current_dir,
    fs::{File, OpenOptions},
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

pub fn get_circuit(r1cs: &str, num_of_wires: usize, witness: &mut WitnessMap) -> IoResult<Circuit> {
    let mut tmp_wit_index = num_of_wires as u32;
    let reader = OpenOptions::new().read(true).open(r1cs)?;
    // let reader = BufReader::new(reader);
    let r1cs_file: R1CSFile<ark_bn254::Bn254> = R1CSFile::new(reader)?;
    let circuit = circom_circuit::acir_circuit_from_r1cs_file(r1cs_file, &mut tmp_wit_index, witness);
    println!("r1cs: {:?}", r1cs);
    Ok(circuit)
}

pub fn get_witness(wtns: &str) -> WitnessMap {
    circom_witness::read_file(wtns)
}

pub fn get_circom(r1cs: &str, wtns: &str) -> IoResult<(Circuit, WitnessMap)> {
    let mut witness = get_witness(wtns);
    let num_of_wires = witness.clone().into_iter().count();
    let circuit = get_circuit(r1cs, num_of_wires, &mut witness)?;
    Ok((circuit, witness))
}