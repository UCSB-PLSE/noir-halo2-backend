mod fs {
    use std::{
        env::var,
        fs::{self, File},
        io::{BufReader, BufWriter}
    };
    
    use pse_halo2_proofs::{
        halo2curves::{
            bn256::{Bn256, G1Affine},
            CurveAffine,
        },
        poly::{
            commitment::{Params, ParamsProver},
            kzg::commitment::ParamsKZG,
        },
    };
    use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

    /// Reads the srs from a file found in `./params/kzg_bn254_{k}.srs` or `{dir}/kzg_bn254_{k}.srs` if `PARAMS_DIR` env var is specified.
    /// * `k`: degree that expresses the size of circuit (i.e., 2^<sup>k</sup> is the number of rows in the circuit)
    pub fn read_params(k: u32) -> ParamsKZG<Bn256> {
        let dir = var("PARAMS_DIR").unwrap_or_else(|_| "./params".to_string());
        ParamsKZG::<Bn256>::read(&mut BufReader::new(
            File::open(format!("{dir}/kzg_bn254_{k}.srs").as_str())
                .expect("Params file does not exist"),
        ))
        .unwrap()
    }
    
    /// Attempts to read the srs from a file found in `./params/kzg_bn254_{k}.srs` or `{dir}/kzg_bn254_{k}.srs` if `PARAMS_DIR` env var is specified, creates a file it if it does not exist.
    /// * `k`: degree that expresses the size of circuit (i.e., 2^<sup>k</sup> is the number of rows in the circuit)
    /// * `setup`: a function that creates the srs
    pub fn read_or_create_srs<'a, C: CurveAffine, P: ParamsProver<'a, C>>(
        k: u32,
        setup: impl Fn(u32) -> P,
    ) -> P {
        let dir = var("PARAMS_DIR").unwrap_or_else(|_| "./params".to_string());
        let path = format!("{dir}/kzg_bn254_{k}.srs");
        match File::open(path.as_str()) {
            Ok(f) => {
                #[cfg(feature = "display")]
                println!("read params from {path}");
                let mut reader = BufReader::new(f);
                P::read(&mut reader).unwrap()
            }
            Err(_) => {
                #[cfg(feature = "display")]
                println!("creating params for {k}");
                fs::create_dir_all(dir).unwrap();
                let params = setup(k);
                params.write(&mut BufWriter::new(File::create(path).unwrap())).unwrap();
                params
            }
        }
    }
    
    /// Generates the SRS for the KZG scheme and writes it to a file found in "./params/kzg_bn2_{k}.srs` or `{dir}/kzg_bn254_{k}.srs` if `PARAMS_DIR` env var is specified, creates a file it if it does not exist"
    /// * `k`: degree that expresses the size of circuit (i.e., 2^<sup>k</sup> is the number of rows in the circuit)
    pub fn gen_srs(k: u32) -> ParamsKZG<Bn256> {
        read_or_create_srs::<G1Affine, _>(k, |k| {
            ParamsKZG::<Bn256>::setup(k, ChaCha20Rng::from_seed(Default::default()))
        })
    }
}

#[cfg(test)]
mod test {
    use super::fs;
    use ark_bn254::Bn254;
    use ark_ec::{bls12::Bls12, pairing::Pairing};
    use circ::{ir::term::{serde_mods::vec, text::parse_value_map}, target::{aby::trans, r1cs::{opt::reduce_linearities, trans::to_r1cs}}};
    use rand::{distributions::WeightedError, rngs::StdRng};
    use core::num;
    use std::{collections::HashMap, fmt::Debug, hash::BuildHasherDefault};
    use crate::{
        circom, circuit_translator::NoirHalo2Translator, dimension_measure::DimensionMeasurement, r1cs_reader::{ConstraintVec, R1CS},
    };
    use acvm::{acir::{circuit, native_types::{Witness, WitnessMap}}, FieldElement};
    use noir_halo2_backend_common::test_helpers::build_artifacts;
    use pse_halo2wrong::{
        curves::bn256::Fr,
        halo2::{
            dev::{FailureLocation, MockProver, VerifyFailure},
            plonk::Any,
        },
    };
    use rand::SeedableRng;
    use std::marker::PhantomData;
    
    use regex::Regex;
    use pse_halo2_proofs::{
        dev::{CircuitCost},
        plonk::{create_proof, Circuit, keygen_vk, keygen_pk, verify_proof, ProvingKey, VerifyingKey},
        poly::kzg::{
            commitment::KZGCommitmentScheme, commitment::ParamsKZG, multiopen::ProverSHPLONK,
            multiopen::VerifierSHPLONK, strategy::SingleStrategy,
        },
        poly::commitment::ParamsProver,
        halo2curves::{bn256::{Bn256, G1Affine, G2}, group::prime::PrimeGroup},
        transcript::{
            Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
        },
    };
    
    #[test]
    // fn read_r1cs() {
    //     let r1cs = "/Users/work/git/halo2/halo2_backend/example/circuit.r1cs";
    //     let circuit = circom::get_circuit(r1cs, 0).unwrap();
    // }

    #[test]
    fn read_wtns() {
        let wtns = "/Users/work/git/halo2/halo2_backend/example/circuit_js/witness.wtns";
        let w = circom::get_witness(wtns);
        for (i, f) in w {
            println!("{:?}: {:?}", i, f);
        }
    }

    use std::{hash::Hash, iter::Product,time::Instant};
    use ark_std::{end_timer, perf_trace::TimerInfo, start_timer, Zero};

     /// Helper function to generate a proof with real prover using SHPLONK KZG multi-open polynomical commitment scheme
    /// and Blake2b as the hash function for Fiat-Shamir.
    pub fn gen_proof_with_instances(
        params: &ParamsKZG<Bn256>,
        pk: &ProvingKey<G1Affine>,
        circuit: impl Circuit<Fr>,
        instances: &[&[Fr]],
    ) -> Vec<u8> {
        let rng = StdRng::seed_from_u64(0);
        let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
        create_proof::<
            KZGCommitmentScheme<Bn256>,
            ProverSHPLONK<'_, Bn256>,
            Challenge255<_>,
            _,
            Blake2bWrite<Vec<u8>, G1Affine, _>,
            _,
        >(params, pk, &[circuit], &[instances], rng, &mut transcript)
        .expect("prover should not fail");
        transcript.finalize()
    }

    /// For testing use only: Helper function to generate a proof **without public instances** with real prover using SHPLONK KZG multi-open polynomical commitment scheme
    /// and Blake2b as the hash function for Fiat-Shamir.
    pub fn gen_proof(
        params: &ParamsKZG<Bn256>,
        pk: &ProvingKey<G1Affine>,
        circuit: impl Circuit<Fr>,
    ) -> Vec<u8> {
        gen_proof_with_instances(params, pk, circuit, &[&[]])
    }

    /// Helper function to verify a proof (generated using [`gen_proof_with_instances`]) using SHPLONK KZG multi-open polynomical commitment scheme
    /// and Blake2b as the hash function for Fiat-Shamir.
    pub fn check_proof_with_instances(
        params: &ParamsKZG<Bn256>,
        vk: &VerifyingKey<G1Affine>,
        proof: &[u8],
        instances: &[&[Fr]],
        expect_satisfied: bool,
    ) {
        let verifier_params = params.verifier_params();
        let strategy = SingleStrategy::new(params);
        let mut transcript = Blake2bRead::<_, _, Challenge255<_>>::init(proof);
        let res = verify_proof::<
            KZGCommitmentScheme<Bn256>,
            VerifierSHPLONK<'_, Bn256>,
            Challenge255<G1Affine>,
            Blake2bRead<&[u8], G1Affine, Challenge255<G1Affine>>,
            SingleStrategy<'_, Bn256>,
        >(verifier_params, vk, strategy, &[instances], &mut transcript);
        // Just FYI, because strategy is `SingleStrategy`, the output `res` is `Result<(), Error>`, so there is no need to call `res.finalize()`.

        println!("Verification result: {:?}", res);
        if expect_satisfied {
            res.unwrap();
        } else {
            assert!(res.is_err());
        }
    }

    /// For testing only: Helper function to verify a proof (generated using [`gen_proof`]) without public instances using SHPLONK KZG multi-open polynomical commitment scheme
    /// and Blake2b as the hash function for Fiat-Shamir.
    pub fn check_proof(
        params: &ParamsKZG<Bn256>,
        vk: &VerifyingKey<G1Affine>,
        proof: &[u8],
        expect_satisfied: bool,
    ) {
        check_proof_with_instances(params, vk, proof, &[&[]], expect_satisfied);
    }

    #[derive(Debug)]
    struct Cost <G: PrimeGroup, PlafH2Circuit: Circuit<G::Scalar>>{
        // mockprover_verified: bool,
        // Circuit cost
        circuit_cost: CircuitCost::<G, PlafH2Circuit>,
        // Time cost
        vk_time: f64,
        pk_time: f64,
        proof_time: f64,
        proof_size: usize,
        verify_time: f64,
    }

    fn extract_usize_field<T: Debug>(obj: &T, field: &str) -> Option<usize> {
        let debug_str = format!("{:?}", obj);
        let re = Regex::new(&format!(r#"{field}:\s*(\d+)"#, field = field)).ok()?;
        let caps = re.captures(&debug_str)?;
        let val_str = caps.get(1)?.as_str();
        val_str.parse::<usize>().ok()
    }

    use circ::ir::{opt::{opt, Opt}, term::{Computations, Value, Value::Field}};
    use circ::front::{Mode, FrontEnd};
    use circ::cfg::{CircCfg, cfg};
    use circ_opt::{CircOpt, BuiltinField, FieldOpt};
    use std::path::PathBuf;
    use circ_fields::{FieldT};
    use circ::front::zsharp::{self, ZSharpFE};
    
    #[test]
    fn circ() {
        use crate::circ_translator::{transform_circ_r1cs_to_r1cs, get_witness, get_circuit};
        
        let mut proof_times = Vec::new();
        let mut verify_times = Vec::new();
        let mut ks = Vec::new();
        let mut proof_sizes = Vec::new();
        let mut num_of_rows = Vec::new();
        // let mut num_of_columns :Vec<usize>= Vec::new();
        
        let test_dirs_names = vec![
            "fib",
            "check_for_collision",
            "check_ship_range",
            "has_ship",
            "check_hit",
            "dotproduct",
            "check_cards",
            "sort"
        ];
        
        let opt_circ = CircOpt::default();
        circ::cfg::set(&opt_circ);
        let mode = Mode::Proof; 
            
        for benchmark in &test_dirs_names {
            let mut path = PathBuf::new();
            let mut inputs_path = PathBuf::new();
            // let mut claimed_ret_path = PathBuf::new();
            
            path.push(format!("circ_example/{}/src.zok", benchmark));
            inputs_path.push(format!("circ_example/{}/wit.pin", benchmark));
            let inputs = zsharp::Inputs {
                file: path,
                mode,
            };
            let mut inputs_map = parse_value_map(&std::fs::read(inputs_path).unwrap());
            
            let cs = ZSharpFE::gen(inputs);
            let cs = opt(cs, 
                vec![
                    Opt::ScalarizeVars,
                    Opt::Flatten,
                    Opt::Sha,
                    Opt::ConstantFold(Box::new([])),
                    Opt::Flatten,
                    Opt::Inline,
                    // Tuples must be eliminated before oblivious array elim
                    Opt::Tuple,
                    Opt::ConstantFold(Box::new([])),
                    Opt::Obliv,
                    // The obliv elim pass produces more tuples, that must be eliminated
                    Opt::Tuple,
                    Opt::LinearScan,
                    // The linear scan pass produces more tuples, that must be eliminated
                    Opt::Tuple,
                    Opt::Flatten,
                    Opt::ConstantFold(Box::new([])),
                    Opt::Inline,
                ],
            );
    
            let (mut prover_data, verifier_data) = to_r1cs(cs.get("main").clone(), cfg());
            prover_data.r1cs = reduce_linearities(prover_data.r1cs, cfg());
            // println!("r1cs: {:#?}", prover_data.r1cs);
    
            let r1cs = transform_circ_r1cs_to_r1cs(&prover_data.r1cs);
            println!("num of variables: {}", r1cs.num_variables);
            println!("num of aux: {}", r1cs.num_aux);   
            println!("num of inputs: {}", r1cs.num_inputs);   
            
            let new_map = prover_data.precompute.eval(&inputs_map);
            prover_data.r1cs.check_all(&new_map);
            
            let mut witness_values = get_witness(&new_map, &prover_data);
            let mut circuit = get_circuit(&r1cs, &prover_data, &mut witness_values);
            
            let translator =
                NoirHalo2Translator::<Fr> { circuit, witness_values, _marker: PhantomData::<Fr> };
            let dimension = DimensionMeasurement::measure(&translator).unwrap();
    
            let k = dimension.k();
            let params = fs::gen_srs(k);
            
            let circuit_cost = CircuitCost::<G2, NoirHalo2Translator<Fr>>::measure(k as usize, &translator);
            
            
            println!("circuit cost: {:?}", circuit_cost);
            // Generating vkey
            let vk_start_time = Instant::now();
            let vk = keygen_vk(&params, &translator).unwrap();
            let vk_time = vk_start_time.elapsed();
    
            // Generating pkey
            let pk_start_time = Instant::now();
            let pk = keygen_pk(&params, vk, &translator).unwrap();
            let pk_time = pk_start_time.elapsed();
    
            // Creating the proof
            let proof_start_time = Instant::now();
            let proof = gen_proof(&params, &pk, translator);
            let proof_time = proof_start_time.elapsed();
            let proof_size = proof.len();
    
            // Verifying
            let verify_start_time = Instant::now();
            check_proof(&params, pk.get_vk(), &proof, true);
            let verify_time = verify_start_time.elapsed();
    
            let cost = Cost {
                // mockprover_verified: true,
                circuit_cost: circuit_cost,
                pk_time: pk_time.as_secs_f64(),
                vk_time: vk_time.as_secs_f64(),
                proof_time: proof_time.as_secs_f64(),
                proof_size: proof_size,
                verify_time: verify_time.as_secs_f64(),
            };
            
            proof_times.push(cost.proof_time);
            verify_times.push(cost.verify_time);
            ks.push(k);
            proof_sizes.push(cost.proof_size);
            num_of_rows.push(2_i32.pow(k));
            // num_of_columns.push(extract_usize_field(&cost.circuit_cost, "num_fixed_columns").unwrap() +
            //     extract_usize_field(&cost.circuit_cost, "num_advice_columns").unwrap() +
            //     extract_usize_field(&cost.circuit_cost, "num_instance_columns").unwrap());
        }

        println!(
            "program, k, num_of_rows, num_of_columns, proof_time, proof_size, verify_time"
        );
        for (i, program) in test_dirs_names.iter().enumerate() {
            println!(
                "{},{},{},{},{},{},{}",
                program,
                ks[i],
                num_of_rows[i],
                num_of_columns[i],
                proof_times[i],
                proof_sizes[i],
                verify_times[i]
            );
        }
    }

    #[test]
    fn circom() {
        use std::env;
        use std::process::{Command, Output};
        use std::process::Stdio;

        let args: Vec<String> = env::args().collect();
        let benchmark_default = String::from("simple");

        let test_dirs_names = vec![
            "BigAddNoCarry",
            "BigIsEqual",
            "BigLessThan",
            "BigSub",
            "BinSum",
            "Decoder",
            "MultiAND",
            "MultiMux",
            "num2bits",
            "BigAdd",
            "BigModSumThree",
            "BigModSumFour",
            "ModProd",
            "BigSubModP",
        ];
        
        let mut proof_times = Vec::new();
        let mut verify_times = Vec::new();
        let mut ks = Vec::new();
        let mut proof_sizes = Vec::new();
        let mut num_of_rows = Vec::new();
        // let mut num_of_columns :Vec<usize>= Vec::new();
        // let mut is_installed = false;

        for benchmark in &test_dirs_names {            
            let path =
                std::fs::canonicalize(format!("./example/{benchmark}/circuit.circom"))
                    .unwrap();
            let path = path.to_str().unwrap();
            
            println!("Compiling circom circuit: {}", path);
            let status = Command::new("circom")
                .arg(path)
                .arg("--r1cs")
                .arg("--wasm")
                .arg("--sym")
                .arg("--c")
                .arg("--output")
                .arg(format!("./example/{benchmark}/"))
                .stdout(Stdio::null())
                .status()
                .expect("failed to execute circom");

            if status.success() {
                println!("circom compiled successfully");
            } else {
                eprintln!("circom failed with status: {:?}", status);
            }
            
            // Canonicalize paths for node command
            let generate_witness = std::fs::canonicalize(format!("./example/{benchmark}/circuit_js/generate_witness.js"))
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();

            let wasm_path = std::fs::canonicalize(format!("./example/{benchmark}/circuit_js/circuit.wasm"))
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();

            let input_json = std::fs::canonicalize(format!("./example/{benchmark}/input.json"))
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();

            let witness_out = std::fs::canonicalize(format!("./example/{benchmark}/circuit_js"))
                .unwrap()
                .join("witness.wtns")
                .to_str()
                .unwrap()
                .to_string();

            let status = Command::new("node")
                .arg(generate_witness)
                .arg(wasm_path)
                .arg(input_json)
                .arg(witness_out)
                .stdout(Stdio::null())
                .status()
                .expect("failed to execute generate_witness.js");

            if status.success() {
                println!("witness generated successfully");
            } else {
                eprintln!("witness generation failed with status: {:?}", status);
            }    
                
            let r1cs = format!("example/{}/circuit.r1cs", benchmark).clone();
            let r1cs = r1cs.as_str();
            let wtns = format!("example/{}/circuit_js/witness.wtns", benchmark).clone();
            let wtns = wtns.as_str();
            
            // get circuit
            let (circuit, witness_values) = circom::get_circom(r1cs, wtns).unwrap();

            // println!("witness values: {:?}", witness_values);

            // instantiate halo2 circuit
            let translator =
                NoirHalo2Translator::<Fr> { circuit, witness_values, _marker: PhantomData::<Fr> };
            let dimension = DimensionMeasurement::measure(&translator).unwrap();

            let k = dimension.k();
            let params = fs::gen_srs(k);
            
            let circuit_cost = CircuitCost::<G2, NoirHalo2Translator<Fr>>::measure(k as usize, &translator);
                
            // Generating vkey
            let vk_start_time = Instant::now();
            let vk = keygen_vk(&params, &translator).unwrap();
            let vk_time = vk_start_time.elapsed();

            // Generating pkey
            let pk_start_time = Instant::now();
            let pk = keygen_pk(&params, vk, &translator).unwrap();
            let pk_time = pk_start_time.elapsed();

            // Creating the proof
            let proof_start_time = Instant::now();
            let proof = gen_proof(&params, &pk, translator);
            let proof_time = proof_start_time.elapsed();
            let proof_size = proof.len();

            // Verifying
            let verify_start_time = Instant::now();
            check_proof(&params, pk.get_vk(), &proof, true);
            let verify_time = verify_start_time.elapsed();

            let cost = Cost {
                // mockprover_verified: true,
                circuit_cost: circuit_cost,
                pk_time: pk_time.as_secs_f64(),
                vk_time: vk_time.as_secs_f64(),
                proof_time: proof_time.as_secs_f64(),
                proof_size: proof_size,
                verify_time: verify_time.as_secs_f64(),
            };
            
            proof_times.push(cost.proof_time);
            verify_times.push(cost.verify_time);
            ks.push(k);
            proof_sizes.push(cost.proof_size);
            num_of_rows.push(2_i32.pow(k));
            // num_of_columns.push(extract_usize_field(&cost.circuit_cost, "num_fixed_columns").unwrap() +
            //     extract_usize_field(&cost.circuit_cost, "num_advice_columns").unwrap() +
            //     extract_usize_field(&cost.circuit_cost, "num_instance_columns").unwrap());
        }

        println!(
            "program, k, num_of_rows, num_of_columns, proof_time, proof_size, verify_time"
        );
        for (i, program) in test_dirs_names.iter().enumerate() {
            println!(
                "{},{},{},{},{},{},{}",
                program,
                ks[i],
                num_of_rows[i],
                num_of_columns[i],
                proof_times[i],
                proof_sizes[i],
                verify_times[i]
            );
        }
    }


        // let benchmark = args.get(3).unwrap_or(&benchmark_default);
        // let r1cs = format!("example/{}/circuit.r1cs", benchmark).clone();
        // let r1cs = r1cs.as_str();
        // let wtns = format!("example/{}/circuit_js/witness.wtns", benchmark).clone();
        // let wtns = wtns.as_str();
        
        // // get circuit
        // let (circuit, witness_values) = circom::get_circom(r1cs, wtns).unwrap();

        // // for (i, f) in witness_values.clone() {
        // //     println!("{:?}: {:?}", i, f);
        // // }
        
        // // println!("circuit: {:?}", circuit);
        // // instantiate halo2 circuit
        // let translator =
        //     NoirHalo2Translator::<Fr> { circuit, witness_values, _marker: PhantomData::<Fr> };
        // let dimension = DimensionMeasurement::measure(&translator).unwrap();

        // let instance = vec![vec![]];

        // // run mock prover expecting success
        // let prover = MockProver::run(dimension.k(), &translator, instance).unwrap();
        // assert_eq!(prover.verify(), Ok(()));


    #[test]
    fn test_public_io_circuit_success() {
        // get circuit
        let (circuit, witness_values) = build_artifacts("10_public_io", "pse_halo2_backend");

        // instantiate halo2 circuit
        let translator =
            NoirHalo2Translator::<Fr> { circuit, witness_values, _marker: PhantomData::<Fr> };
        let dimension = DimensionMeasurement::measure(&translator).unwrap();

        // instance value (known to be 7)
        let instance = vec![Fr::from_raw([7u64, 0, 0, 0])];

        // run mock prover expecting success
        let prover = MockProver::run(dimension.k(), &translator, vec![instance]).unwrap();
        assert_eq!(prover.verify(), Ok(()));
    }

    #[test]
    fn test_public_io_circuit_fail_instance() {
        // get circuit
        let (circuit, witness_values) = build_artifacts("10_public_io", "pse_halo2_backend");

        // instantiate halo2 circuit
        let translator =
            NoirHalo2Translator::<Fr> { circuit, witness_values, _marker: PhantomData::<Fr> };
        let dimension = DimensionMeasurement::measure(&translator).unwrap();

        // instance value (known to be 7, incorrectly set to 8)
        let instance = vec![Fr::from_raw([8u64, 0, 0, 0])];

        // define permutation error expected when instance value is not set or incorrect
        let permutation_error = Err(vec![
            VerifyFailure::Permutation {
                column: (Any::advice(), 0).into(),
                location: FailureLocation::InRegion { region: (7, "region 0").into(), offset: 0 },
            },
            VerifyFailure::Permutation {
                column: (Any::Instance, 0usize).into(),
                location: FailureLocation::OutsideRegion { row: 0 },
            },
        ]);

        // run mock prover with incorrect instance expecting permutation failure
        let prover = MockProver::run(dimension.k(), &translator, vec![instance]).unwrap();
        assert_eq!(prover.verify(), permutation_error);

        // run mock prover with no instance expecting permutation failure
        let prover = MockProver::run(dimension.k(), &translator, vec![vec![]]).unwrap();
        assert_eq!(prover.verify(), permutation_error);
    }

    #[test]
    fn test_public_io_circuit_fail_witness() {
        // get circuit
        let (circuit, mut witness_values) = build_artifacts("10_public_io", "pse_halo2_backend");

        // mutate witness to be incorrect
        witness_values.insert(Witness(1), FieldElement::from(5u128));

        // instantiate halo2 circuit
        let translator =
            NoirHalo2Translator::<Fr> { circuit, witness_values, _marker: PhantomData::<Fr> };
        let dimension = DimensionMeasurement::measure(&translator).unwrap();

        // instance value (known to be 7)
        let instance = vec![Fr::from_raw([7u64, 0, 0, 0])];

        // run mock prover expecting success
        // expects [-1(5) + -1(4) + 1(7)] == 0, should be [-1(3) + -1(4) + 1(7)]
        let prover = MockProver::run(dimension.k(), &translator, vec![instance]).unwrap();
        assert_eq!(
            prover.verify(),
            Err(vec![VerifyFailure::ConstraintNotSatisfied {
                constraint: ((0, "main_gate").into(), 0, "").into(),
                location: FailureLocation::InRegion { region: (5, "region 0").into(), offset: 5 },
                cell_values: vec![
                    (((Any::advice(), 0).into(), 0).into(), String::from("0x5")),
                    (((Any::advice(), 1).into(), 0).into(), String::from("0x4")),
                    (((Any::advice(), 2).into(), 0).into(), String::from("0x7")),
                    (((Any::advice(), 3).into(), 0).into(), String::from("0")),
                    (((Any::advice(), 4).into(), 0).into(), String::from("0")),
                    (((Any::advice(), 4).into(), 1).into(), String::from("0")),
                    (((Any::Fixed, 0).into(), 0).into(), String::from("-1")),
                    (((Any::Fixed, 1).into(), 0).into(), String::from("-1")),
                    (((Any::Fixed, 2).into(), 0).into(), String::from("1")),
                    (((Any::Fixed, 3).into(), 0).into(), String::from("0")),
                    (((Any::Fixed, 4).into(), 0).into(), String::from("0")),
                    (((Any::Fixed, 5).into(), 0).into(), String::from("0")),
                    (((Any::Fixed, 6).into(), 0).into(), String::from("0")),
                    (((Any::Fixed, 7).into(), 0).into(), String::from("0")),
                    (((Any::Fixed, 8).into(), 0).into(), String::from("0")),
                ]
            }])
        );
    }

    #[test]
    fn test_circuits_native() {
        let test_dirs_names = vec![
            "1_mul",
            "2_div",
            "3_add",
            "4_sub",
            "5_over",
            "6_array",
            "7_function",
            "8_bit_and",
            "9_poseidon",
        ];
        for program in test_dirs_names {
            // get circuit
            let (circuit, witness_values) = build_artifacts(program, "pse_halo2_backend");

            // instantiate halo2 circuit
            let translator =
                NoirHalo2Translator::<Fr> { circuit, witness_values, _marker: PhantomData::<Fr> };
            let dimension = DimensionMeasurement::measure(&translator).unwrap();

            // run mock prover expecting success
            let prover = MockProver::run(dimension.k(), &translator, vec![vec![]]).unwrap();
            assert_eq!(prover.verify(), Ok(()));
        }
    }
}
