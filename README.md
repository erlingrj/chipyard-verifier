## BOOM Verifier
This program automates the verification and microbenchmarking using Verilator
in Chipyard. It will compile a verilator executable and can run the ISA-test,
microbenchmark and a Spectre-Attack and record the results.

## Usage
- Must be run from inside chipyard/sims/verilator as it uses "make" to build the
verilator executable
```
verify --config SmallBoomConfig --compile --asm --bmark --print --terminate --output boom.out
```
Will compile, run ISA test, run benchmark test, print to terminal and boom.out
and stop execution on first error. 


## What do we want?
- Compile to verify no bugs
- Run ASM tests to verify function
- Run Benchmarks to verify function
- Run Spectre-attack to verify OoO
- Dump a log:
Compile: Duration status
ASM: Status
Benchmark: Status + results
Spectre-attack: status + results


