extern crate clap;
use std::process::{Command, exit};
use std::error::Error;
use std::fs::File;
use std::io::Write;
use clap::{Arg, App};
use glob::glob;
use std::path::Path;
use rayon::prelude::*;
use std::env;



fn main() {

    // Handle command line arguments
    let matches = App::new("BOOM verifier and microbenchmarker")
                            .version("1.0")
                            .author("Erling Rennemo Jellum <erlingrj@stud.ntnu.no>")
                            .about("Compiles, verifies and benchmarks a BOOM design")
                            .arg(Arg::with_name("config")
                                 .short("c")
                                 .long("config")
                                 .required(true)
                                 .takes_value(true)
                                 .help("Specify the config to pass to the build system. E.g. SliceBoomConfig"))
                            .arg(Arg::with_name("compile")
                                 .short("x")
                                 .long("compile")
                                 .help("Should we compile the simulator, or is it already ready and compiled"))
                            .arg(Arg::with_name("asm")
                                 .short("a")
                                 .long("asm")
                                 .help("Run the RISC-V ISA test?"))
                            .arg(Arg::with_name("bmark")
                                 .short("b")
                                 .long("bmark")
                                 .help("Run the Benchmark suite. This will also return the CCs used per program"))
                            .arg(Arg::with_name("spectre")
                                 .short("s")
                                 .long("spectre")
                                 .help("Run Spectre Attack program. The path to the executable must be passed. Will report whether it passed or failed. Presupposes that the Spectre ATtacks return 1 in case of fail. and 0 in case of success")
                                 .takes_value(true))
                            .arg(Arg::with_name("print")
                                 .short("p")
                                 .long("print")
                                 .help("Should results be printed to screen also?"))
                            .arg(Arg::with_name("terminate")
                                 .short("t")
                                 .long("terminate")
                                 .help("Should the program terminate on first error or run to completion"))
                            .arg(Arg::with_name("output")
                                 .short("o")
                                 .long("output")
                                 .required(true)
                                 .help("Path to where to store the output")
                                 .takes_value(true))
                            .get_matches();
    
    
   
    // Get the RISCV environmenal variable to find ASM and BMARKS
    let mut RISCV: Option<String> = None;
    for (key, val) in env::vars() {
        if key == "RISCV" {
            RISCV = Some(String::from(val));
        }
    }
    let BENCHMARK_PATH: String = String::from("/home/erling/riscv-tests/benchmarks");
    let ASM_PATH: String = format!("{}/{}", RISCV.clone().unwrap(), "/riscv64-unknown-elf/share/riscv-tests/isa");


    // unwrap config and output_file. Safe to do since they are required
    let print = matches.is_present("print");
    let config = matches.value_of("config").unwrap();
    let output_path = Path::new(matches.value_of("output").unwrap());
    // Get filedescriptor to the log file
    let  output_file = match File::create(&output_path) {
        Err(why) => panic!("Couldnt create log-file: {}: {}", output_path.display(), why.description()),
        Ok(file) => file,
    };
    let terminate = matches.is_present("terminate");

    if matches.is_present("compile") {
        // Compile simulator
        println!("Building Verilator simulator with CONFIG={}", config);
        let output = Command::new("/usr/bin/make").args (&["-j6",format!("CONFIG={}",config).as_str()])
            .output()
            .expect("Failed to execute make");
        if output.status.success() {
            log("Verilator Build: PASS", Some(&output_file), print);
        }
        else {
            log("Verilator Build: FAIL", Some(&output_file), print);
            log(String::from_utf8(output.stdout).unwrap().as_str(), Some(&output_file), true);
            exit(1);
        }
    }

    // Store the name of the Verilator executable. This is either produced by the compilation step
    //  or should already be present
    let SIMULATOR = format!("simulator-example-{}", config);
    // Verify that we in fact have a simulator
    assert!(Path::new(&SIMULATOR).exists(), "Cannot find verilator executable for that config. Compile needed?");
    
    if matches.is_present("asm") {
        println!("Running ISA Assembly Tests");
        let mut asms: Vec <(std::path::PathBuf, bool)> = Vec::new();
        for entry in glob(format!("{}/rv64*", ASM_PATH).as_str()).expect("Failed to open Assembly path") {
            match entry {
                Ok(path) =>{
                    if !path.to_str().unwrap().ends_with("dump") {
                        asms.push((path, false));
                    }
                }
                
                Err(e) => println!("{:?}", e),
            }
        }

        let asm_results: Vec<(String, bool)> = asms.par_iter()
            .map(|(path, _)| {
                let sim_c = SIMULATOR.clone();
                let path_c = path.clone();
                let program_name = path_to_testname(&path);
                let output = Command::new(format!("./{}", sim_c))
                                          .arg(path_c)
                                          .output()
                                          .expect("Run Asm failed");
                if output.status.success() {
                    (program_name, true)
                } else {
                    if terminate {
                        exit(1);
                    }
                    (program_name, false)
                }

            }
            ).collect();

            for (path, res) in asm_results {
                log(format!("{}: {}", path, match res { true => "PASS", false => "FAIL",}).as_str(), Some(&output_file), print);
                if terminate && !res {
                    exit(1);
                }
            }
    }


    if matches.is_present("bmark") {
        println!("Running Benchmark Suite");
        let mut bmarks: Vec <(std::path::PathBuf, bool, u32, u32)> = Vec::new();
        for entry in glob(format!("{}/*.riscv", BENCHMARK_PATH).as_str()).expect("Failed to open Benchmark test") {
            match entry {
                Ok(path) =>{
                    bmarks.push((path, false, 0, 0));
                }
                
                Err(e) => println!("{:?}", e),
            }
        }

        let bmark_results: Vec<(String, bool, u32, u32, u32, u32)> = bmarks.par_iter()
            .map(|(path, _, _, _)| {
                let sim_c = SIMULATOR.clone();
                let path_c = path.clone();
                let program_name = path_to_testname(&path);
                let output = Command::new(format!("./{}", sim_c))
                                          .arg(path_c)
                                          .output()
                                          .expect("Run benchmark failed");
                if output.status.success() {
                     let (cycles, instr,aq,bq) = parse_bmark_output(output.stdout);
                    (program_name, true, cycles, instr,aq,bq)
                } else {
                    if terminate {
                        exit(1);
                    }

                    (program_name, false, 0, 0,0,0)
                }

            }
            ).collect();

            for (program_name, res, cc, insts,aq,bq) in bmark_results {
                log(format!("{}: {}, CC={}, insts={}, AQ={}, BQ={}",program_name,res, cc,insts,aq,bq).as_str(), Some(&output_file), print);
                if terminate && !res {
                    exit(1);
                }
            }

    }


    if matches.is_present("spectre") {
        println!("Running Spectre Attack");
        let spectre_bin = Path::new(matches.value_of("spectre").unwrap());
        assert!(Path::new(&spectre_bin).exists(), "Cannot find Spectre executable");

         // Run Spectre-attack on simulator
        let output = Command::new(format!("./{}",SIMULATOR))
                                                .arg(spectre_bin)
                                                .output()
                                                .expect("Failed to run Spectre executable");
                        
        if output.status.success() {
            log("Spectre Attack: PASS", Some(&output_file), print);

        } else {
            log("Spectre Attack: FAIL", Some(&output_file), print);
            log(String::from_utf8(output.stdout).unwrap().as_str(), Some(&output_file), print);

            if terminate {
                std::process::exit(1);
            }
        }   

    }
}


// parse_bmark_output takes the raw byte output from a benchmark program
//  and returns (nCycles, nInstr)
fn parse_bmark_output(out: Vec<u8>) -> (u32,u32,u32,u32) {
    let output = String::from_utf8(out).unwrap();
    let mut cycles: u32 = 0;
    let mut insts: u32 = 0;
    let mut aq: u32 = 0;
    let mut bq: u32 = 0;
    for line in output.lines() {
        if line.contains("mcycle") {
            let words: Vec<&str> =  line.split(' ').collect();
            cycles = words[2].parse::<u32>().unwrap();

        } else if line.contains("minstret") {
            let words: Vec<&str> =line.split(' ').collect();
            insts = words[2].parse::<u32>().unwrap();
        } else if line.contains("vvadd") {
            let words: Vec<&str> = line.split(' ').collect();
            cycles = words[7].parse::<u32>().unwrap();
            insts = (cycles as f32 / words[9].parse::<f32>().unwrap()) as u32;
        } else if line.contains("matmul") {
            let words: Vec<&str> = line.split(' ').collect();
            cycles = words[7].parse::<u32>().unwrap();
            insts = (cycles as f32 / words[9].parse::<f32>().unwrap()) as u32;
        } else if line.contains("C0") {
            if line.contains("instructions") {
                let words: Vec<&str> = line.split(' ').collect();
                insts = words[1].parse::<u32>().unwrap();
            } else if line.contains("cycles") {
                let words: Vec<&str> = line.split(' ').collect();
                cycles = words[1].parse::<u32>().unwrap();
            }
        } else if line.contains("AQ") {
            let words: Vec<&str> = line.split(' ').collect();
            aq += words[2].parse::<u32>().unwrap();
        
        } else if line.contains("BQ") {
            let words: Vec<&str> = line.split(' ').collect();
            bq += words[2].parse::<u32>().unwrap();
        }
    }
    (cycles, insts, aq, bq)

}

fn log(line: &str, log_file: Option<&File>, console: bool) {
    match log_file {
        Some(mut file) =>{
            file.write_all(line.as_bytes());
            file.write_all("\n".as_bytes())
            },
        None => Ok(())
    };

    if console {
        println!("{}", line);
    }
}

fn path_to_testname(path: &std::path::PathBuf) -> String {
    // Basically get the last part of the path
    // and return as string
    
    let path_string = String::from((*path).clone().into_os_string().into_string().unwrap());
    String::from(path_string.split('/').last().unwrap())
}

