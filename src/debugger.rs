use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::collections::HashMap;

use crate::inferior::{Inferior,Status};
use crate::debugger_command::DebuggerCommand;
use crate::dwarf_data::{DwarfData, Error as DwarfError};

pub struct Debugger {
    /// The path to the target program
    target: String,
    /// The path to the history filefor command history
    history_path: String,
    /// The readline editor for user input
    readline: Editor<()>,
    /// The currently running inferior process
    inferior: Option<Inferior>,
    /// The debug data obtained from the target program's DWARF information
    debug_data: DwarfData,
    /// The breakpoints set in the target program.
    breakpoints: HashMap<usize, u8>,
    /// The softirq for step over
    step_over_points: HashMap<usize, u8>,
}

impl Debugger {
    /// # brief
    /// Creates a new debugger 
    ///
    /// # param
    /// - `target` : The path to the target program.
    ///
    /// # return
    /// * A new Debug Object
    ///
    pub fn new(target: &str) -> Self {
        let debug_data = match DwarfData::from_file(target) {
            Ok(val) => val,
            Err(DwarfError::ErrorOpeningFile) => {
                println!("Could not open file {}", target);
                std::process::exit(1);
            }
            Err(DwarfError::DwarfFormatError(err)) => {
                println!("Could not debugging system from {}: {:?}", target, err);
                std::process::exit(1);
            }
        };

        debug_data.print();
        let history_path = format!("{}/.deet_history", std::env::var("HOME").unwrap());
        let mut readline = Editor::<()>::new();
        // Attempt to load history from ~/.deet_history if it exists

        let _ = readline.load_history(&history_path);

        let breakpoints = HashMap::new();
        let step_over_points = HashMap::new();
        Debugger {
            target: target.to_string(),
            history_path,
            readline,
            inferior: None,
            debug_data,
            breakpoints,
            step_over_points,
        }
    }

    /// # brief
    /// Get the next debugger command from user input.
    /// The loop waits for user input and handles different situations:
    ///
    /// - If the user presses Ctrl+C, a message is printed and continues to wait for user input.
    /// - If the user presses Ctrl+D (indicating the end of input on some systems), return a `DebuggerCommand::Quit` to exit the debugger.
    /// - If other I/O errors occur, a panic is thrown.
    /// - If the user input is OK, the user input is added to the history and attempts to save the history to a file.
    /// - Next, it splits the user-entered string into words and attempts to parse it into debugger commands. If the command is successfully parsed, the command is returned; otherwise a message is printed indicating that the command was not recognized.
    ///
    /// # return
    /// Returns a `DebuggerCommand` enumeration type representing the next debugger command 
    /// entered by the user.
    fn get_next_command(&mut self) -> DebuggerCommand {
        loop {
            // Print prompt and get next line of user input
            match self.readline.readline("\x1b[35m(deet) \x1b[0m") {
                Err(ReadlineError::Interrupted) => {
                    // User pressed ctrl+c. We're going to ignore it
                    println!("Type\"quit\"to exit");
                }
                Err(ReadlineError::Eof) => {
                    // User pressd ctrl+d, which is the equivalent of "quit" for our purposes
                    return DebuggerCommand::Quit;
                }
                Err(err) => {
                    panic!("Unexpected I/O Error: {:?}", err);
                }
                Ok(line) => {
                    if line.trim().len() == 0 {
                        continue;
                    }
                    self.readline.add_history_entry(line.as_str());
                    if let Err(err) = self.readline.save_history(&self.history_path) {
                        println!("Warning: failed to save history file at {}: {}", 
                            self.history_path,
                            err
                        );
                    }
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if let Some(cmd) = DebuggerCommand::from_tokens(&tokens) {
                        return cmd;
                    } else {
                        println!("Unrecognized command.");
                    }
                }
            }
        }
    }

    /// # brief
    /// Parses a hexadecimal address string and returns the corresponding `usize` value.
    ///
    /// # param
    /// - `addr` - A hexadecimal address string, which may start with "0x" prefix.
    ///
    /// # return
    /// * `Some(usize)` - If the address string is successfully parsed, returns the corresponding `usize` value.
    ///
    /// # Examples
    ///
    /// ```
    /// let parser = AddressParser;
    /// let address_str = "0x12AB";
    /// assert_eq!(parser.parse_address(address_str), Some(4779));
    /// ```
    fn parse_address(&self, addr: &str) -> Option<usize> {
        let addr_without_0x = if addr.to_lowercase().starts_with("0x") {
            &addr[2..]
        } else {
            &addr
        };
        usize::from_str_radix(addr_without_0x, 16).ok()
    }

    /// # brief
    /// Run the debugger, processing user commands and controlling the inferior process.
    ///
    /// This method enters a loop to continuously receive and process user commands for controlling
    /// the debugger and the inferior process. It handles commands such as quitting the debugger,
    /// starting or restarting the inferior process, continuing the execution, printing backtraces,
    /// and setting breakpoints.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut debugger = Debugger::new();
    /// debugger.run();
    /// ```plaintext
    pub fn run(&mut self) {
        loop {
            match self.get_next_command() {

                // if the inferior still alive, then kill it and set inferior into None, finally
                // stop the loop
                DebuggerCommand::Quit               => {
                    if self.inferior.is_some() {
                        self.inferior.as_mut().unwrap().kill();
                        self.inferior = None;
                    }
                    return;
                }

                // Determine whether inferior exists. If it exists, kill it and then 
                // create a new inferior and execute it directly.
                DebuggerCommand::Run(args)             => {
                    if self.inferior.is_some() {
                        // there is already a inferior running
                        // if it has not exited, kill it first
                        self.inferior.as_mut().unwrap().kill();
                        self.inferior = None;
                    }
                    if let Some(inferior) = Inferior::new(&self.target, &args, &mut self.breakpoints) {
                        // Crate the inferior
                        self.inferior = Some(inferior);

                        match self.inferior.as_mut().unwrap().continue_run(None, &self.breakpoints, &mut self.step_over_points).unwrap() {
                            Status::Exited(exit_code)    => {
                                println!("Chlid exited (status {})", exit_code);
                                self.inferior = None;
                            }
                            Status::Signaled(signal)     => {
                                println!("Child exited due to signal {}", signal);
                                self.inferior = None;
                            }
                            Status::Stopped(signal, rip) => {
                                println!("Child stopped (signal {})", signal);
                                let _line = self.debug_data.get_line_from_addr(rip);
                                let _func = self.debug_data.get_function_from_addr(rip);
                                if _line.is_some() && _func.is_some(){
                                    println!("Stopped at {} ({})", _func.unwrap(), _line.unwrap());
                                }
                            }
                        }
                    } else {
                        println!("Error starting subprocess");
                    }
                }

                // call continues_run from inferior ;
                // and wait for status changing of child .
                DebuggerCommand::Continue              => {
                    if self.inferior.is_none() {
                       println!("Error: you can not use continue when there is no process running!");
                    } else {
                        match self.inferior.as_mut().unwrap().continue_run(None, &self.breakpoints, &mut self.step_over_points).unwrap() {
                            Status::Exited(exit_code) => {
                                self.inferior = None;
                                println!("Child exit (status {})", exit_code);
                            }
                            Status::Signaled(single) => {
                                self.inferior = None;
                                println!("Child exited due to signal {}", single);
                            }
                            Status::Stopped(single, rip) => {
                                println!("Child stopped (signal {})", single);
                                let _line = self.debug_data.get_line_from_addr(rip);
                                let _func = self.debug_data.get_function_from_addr(rip);
                                if _line.is_some() && _func.is_some(){
                                    println!("Stopped at {} ({})", _func.unwrap(), _line.unwrap());
                                }
                            }
                        }
                    }
                }

                // Use the ptracer::step() function to execute 
                // one step downward from the current rip then 
                // and observe the state changes of the child process
                DebuggerCommand::Step                  => {
                    if self.inferior.is_none() {
                        println!("Error: you can not use step when there is no process running");
                    } else {
                        match self.inferior.as_mut().unwrap().step_over(&self.breakpoints, &mut self.step_over_points, None, &self.debug_data).unwrap() {
                            Status::Exited(exit_code)    => {
                                println!("Chlid exited (status {})", exit_code);
                                self.inferior = None;
                            }
                            Status::Signaled(signal)     => {
                                println!("Child exited due to signal {}", signal);
                                self.inferior = None;
                            }
                            Status::Stopped(signal, rip) => {
                                println!("Child stopped (signal {})", signal);
                                let _line = self.debug_data.get_line_from_addr(rip);
                                let _func = self.debug_data.get_function_from_addr(rip);
                                if _line.is_some() && _func.is_some(){
                                    println!("Stopped at {} ({})", _func.unwrap(), _line.unwrap());
                                }
                            }
                        }
                    }
                }

                // print backtrace of this process , untill back to main function
                DebuggerCommand::Backtrace             => {
                    if self.inferior.is_none() {
                        println!("Erro: you can not use backtrace when there is no process running");
                    } else {
                        self.inferior.as_mut().unwrap().print_backtrace(&self.debug_data).unwrap();
                    }
                }

                // judge if the input have'not error , then get this input and parse into address
                // and insert HashMap ( usize(addr) - u8(ori_byte) )
                DebuggerCommand::Breakpoint(localtion) => {
                    let breakpoint_addr;
                    if localtion.starts_with("*") {
                        if let Some(address) = self.parse_address(&localtion[1..]) {
                            breakpoint_addr = address;
                        } else {
                            println!("Invalid address");
                            continue;
                        }
                    } else if let Some(line) = usize::from_str_radix(&localtion, 10).ok() {
                        if let Some(address) = self.debug_data.get_addr_for_line(None, line) {
                            breakpoint_addr = address;
                        } else {
                            println!("Invalid line number");
                            continue;
                        }
                    } else if let Some(address) = self.debug_data.get_addr_for_function(None, &localtion) {
                        breakpoint_addr = address;
                    } else {
                        println!("Usage b|break|breakpoint *address|line|func");
                        continue;
                    }

                    if self.inferior.is_some() {
                        if let Some(instruction) = self.inferior.as_mut().unwrap().write_byte(breakpoint_addr, 0xcc).ok() {
                            println!("Set breakpoint {} at {:#x}", self.breakpoints.len(), breakpoint_addr);
                            self.breakpoints.insert(breakpoint_addr, instruction);
                        } else {
                            println!("Invalid breakpoint address {:#x}", breakpoint_addr);
                        }
                    } else {
                        // when the inferior is initiated, these breakpoints will be installed
                        println!("Set breakpoint {} at {:#x}", self.breakpoints.len(), breakpoint_addr);
                        self.breakpoints.insert(breakpoint_addr, 0);
                    }
                }
            }
        }
    }
}
