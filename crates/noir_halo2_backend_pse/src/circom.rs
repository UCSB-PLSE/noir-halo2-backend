use super::{
    circom_circuit, circom_witness,
    r1cs_reader::{IoResult, R1CSFile},
};
use acvm::acir::{circuit::Circuit, native_types::WitnessMap};
use std::{
    collections::BTreeMap,
    env::current_dir,
    fs::{File, OpenOptions},
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

pub fn get_circuit(r1cs: &str) -> IoResult<Circuit> {
    let reader = OpenOptions::new().read(true).open(r1cs)?;
    // let reader = BufReader::new(reader);
    let r1cs_file: R1CSFile<ark_bn254::Bn254> = R1CSFile::new(reader)?;
    let circuit = circom_circuit::acir_circuit_from_r1cs_file(r1cs_file);
    println!("r1cs: {:?}", r1cs);
    Ok(circuit)
}

pub fn get_witness(wtns: &str) -> WitnessMap {
    circom_witness::read_file(wtns)
}

pub fn get_circom(r1cs: &str, wtns: &str) -> IoResult<(Circuit, WitnessMap)> {
    let circuit = get_circuit(r1cs)?;
    let witness = get_witness(wtns);
    Ok((circuit, witness))
}
