//! DOMS JavaScript Engine Command-Line Interface and REPL.

use std::env;
use std::fs;
use std::io::{self, Write};

mod test262_runner;
use test262_runner::{RunnerConfig, Test262Runner};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        run_repl();
        return;
    }

    match args[1].as_str() {
        "-v" | "--version" => {
            println!("DOMS.js v0.1.0 (Phase 3)");
        }
        "-h" | "--help" => {
            print_help();
        }
        "test262" => {
            let mut config = RunnerConfig::default();
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "-s" | "--suite" => {
                        if i + 1 < args.len() {
                            config.suite_filter = Some(args[i + 1].clone());
                            i += 1;
                        }
                    }
                    "-f" | "--filter" => {
                        if i + 1 < args.len() {
                            config.path_filter = Some(args[i + 1].clone());
                            i += 1;
                        }
                    }
                    "-m" | "--max" => {
                        if i + 1 < args.len() {
                            if let Ok(n) = args[i + 1].parse::<usize>() {
                                config.max_tests = Some(n);
                            }
                            i += 1;
                        }
                    }
                    "-t" | "--threads" => {
                        if i + 1 < args.len() {
                            if let Ok(t) = args[i + 1].parse::<usize>() {
                                config.threads = t;
                            }
                            i += 1;
                        }
                    }
                    "-v" | "--verbose" => {
                        config.verbose = true;
                    }
                    pos_arg if !pos_arg.starts_with('-') => {
                        config.suite_filter = Some(pos_arg.to_string());
                    }
                    _ => {}
                }
                i += 1;
            }
            let runner = Test262Runner::new(config);
            runner.run();
        }
        "eval" | "-e" => {
            if args.len() < 3 {
                eprintln!("Error: 'eval' requires code to evaluate");
                std::process::exit(1);
            }
            execute_code(&args[2]);
        }
        "run" => {
            if args.len() < 3 {
                eprintln!("Error: 'run' requires a file path");
                std::process::exit(1);
            }
            run_file(&args[2]);
        }
        filepath => {
            run_file(filepath);
        }
    }
}

fn print_help() {
    println!("DOMS JavaScript Engine CLI");
    println!("Usage:");
    println!("  doms                  Start interactive REPL");
    println!("  doms run <file.js>    Execute a JavaScript file");
    println!("  doms eval \"<code>\"    Evaluate JavaScript code string");
    println!("  doms test262 [opts]   Run official Test262 ECMAScript conformance test suite");
    println!("  doms --version        Show version information");
    println!("  doms --help           Show this help message");
}

fn execute_code(source: &str) {
    match doms_parser::parse(source) {
        Ok(program) => match doms_compiler::compile(&program) {
            Ok(chunk) => {
                let mut vm = doms_vm::Vm::new();
                match vm.run(&chunk) {
                    Ok(val) => {
                        println!("{}", vm.to_string_value(val));
                    }
                    Err(e) => {
                        eprintln!("Runtime error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("Compile error: {}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_file(filepath: &str) {
    match fs::read_to_string(filepath) {
        Ok(content) => execute_code(&content),
        Err(e) => {
            eprintln!("Failed to read file '{}': {}", filepath, e);
            std::process::exit(1);
        }
    }
}

fn run_repl() {
    println!("DOMS.js v0.1.0 Interactive REPL");
    println!("Type JavaScript expressions or statements. Press Ctrl+C or Ctrl+D to exit.\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut vm = doms_vm::Vm::new();

    loop {
        print!("doms> ");
        if stdout.flush().is_err() {
            break;
        }

        let mut line = String::new();
        match stdin.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed == "exit" || trimmed == "quit" {
                    break;
                }

                match doms_parser::parse(trimmed) {
                    Ok(program) => match doms_compiler::compile(&program) {
                        Ok(chunk) => match vm.run(&chunk) {
                            Ok(val) => println!("{}", vm.to_string_value(val)),
                            Err(e) => eprintln!("Runtime error: {}", e),
                        },
                        Err(e) => eprintln!("Compile error: {}", e),
                    },
                    Err(e) => eprintln!("Parse error: {}", e),
                }
            }
            Err(_) => break,
        }
    }
}
