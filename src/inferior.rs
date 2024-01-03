use nix::sys::ptrace;
use nix::sys::signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use std::process::Child;
use std::process::Command;
use std::os::unix::process::CommandExt;
use std::mem::size_of;
use std::collections::HashMap;
use std::fmt;

use crate::dwarf_data::DwarfData;
use crate::dwarf_data::Line;

/// # brief 
/// Align the given address to the nearest word boundary, Pointer size depends on current platform.
///
/// # param
/// - `addr`: address to be aligned
///
/// # return
/// * Return the aligned address
///
/// # example
/// ```
/// let addr = 0x11;
/// let aligned_addr = align_addr_to_word(addr);
/// println!("addr which was aligned: 0x{:x}", aligned_addr);
/// ```plaintext
fn align_addr_to_word(addr: usize) -> usize {
    addr & (-(size_of::<usize>() as isize) as usize)
}

// Status of the Child Process 
pub enum Status {
    /// Indicates inferior stopped. Contains the signal that stopped the process, as well as the
    /// current instruction pointer that it is stopped at.
    Stopped(signal::Signal, usize),

    /// Indicates inferior exited normally. Contains the exit status code.
    Exited(i32),

    /// Indicates the inferior exited due to signal. Contains the signal that killed the process
    Signaled(signal::Signal),
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Status::Stopped(signal, ip) => write!(f, "Stopped: Signal {:?}, Instruction Pointer: 0x{:X}", signal, ip),
            Status::Exited(exit_code) => write!(f, "Exited with status code: {}", exit_code),
            Status::Signaled(signal) => write!(f, "Signaled: Signal {:?}", signal),
        }
    }
}

/// # brief
/// - Allow father process trace its child process(this function caller)
/// - Attempts to start a new inferior process. Returns Some(Inferior) if successful, or None if
/// an error is encountered.
///
/// # return
/// * Return the aligned address
fn child_traceme() -> Result<(), std::io::Error> {
    ptrace::traceme().or(Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "ptrace TRACEME failed",
    )))
}

pub struct Inferior {
    child: Child,
}

impl Inferior {
    /// # brief
    /// Attempts to start a new inferior process. Returns Some(Inferior) if successful, or None if
    /// an error is encountered.
    ///
    pub fn new(target: &str, args: &Vec<String>, breakpoints: &mut HashMap<usize, u8>) -> Option<Self> {
        let mut cmd = Command::new(target);
        cmd.args(args);
        unsafe {
            // Allow father Process trace chlid ; before execute Child
            cmd.pre_exec(child_traceme);
        }
        // When a process that has PTRACE_TRACEME enabled calls exec,
        // the operating system will local the specified program into process,
        // and then (before the new program starts running) it will pause the process using 
        // SIGTRAP . So at the time when inferior is returnd, chlid process is paused.
        let child_cmd = cmd.spawn().ok()?;
        let mut inferior = Inferior {child: child_cmd};
        // install breakpoints
        let bps = breakpoints.clone();
        for bp in bps.keys() {
            // a set containing all keys. 
            // Traversing this set can obtain the memory address of each breakpoint.
            match inferior.write_byte(*bp, 0xcc) {
                Ok(ori_instr) => {breakpoints.insert(*bp, ori_instr);},
                Err(_) => println!("Invalid breakpoint address {:#x}", bp),
            }
        }
        Some(inferior)
    }

    /// # brief
    /// get pid from io and return it
    ///
    /// # return
    /// * Return the pid of the inferior
    ///
    /// # example
    /// ```
    /// inferior.pid();
    /// ```
    pub fn pid(&self) -> Pid {
        nix::unistd::Pid::from_raw(self.child.id() as i32)
    }

    /// # brief
    /// Kill the process and wait for it to end.
    ///
    /// # example
    /// ```
    /// inferior.kill();
    /// ```
    ///
    pub fn kill(&mut self) {
        self.child.kill().unwrap();
        self.wait(None).unwrap();
        println!("killing running inferior (pid{})", self.pid());
    }

    /// # brief
    /// Waits for the status of the process and returns the corresponding status value.
    ///
    /// # param
    ///  - `option`: Option<WaitPidFlag> - used to specify the behavior of the waiting process. 
    ///  If `None`, the default options are used.
    ///
    /// # return
    /// * If the wait is successful, the process's status value is returned, 
    /// otherwise a `nix::Error` is returned.
    ///
    /// # example
    /// ```
    /// let status = match process.wait(Some(WaitPidFlag::WNOHANG)) {
    /// Ok(s) => s,
    /// Err(e) => return Err(e),
    /// };
    /// ```
    pub fn wait(&self, option: Option<WaitPidFlag>) -> Result<Status, nix::Error> {
        Ok(match waitpid(self.pid(), option)? {
            WaitStatus::Exited(_pid, exit_code) => Status::Exited(exit_code),
            WaitStatus::Signaled(_pid, signal, _core_dumped) => Status::Signaled(signal),
            WaitStatus::Stopped(_pid, signal) => {
                let regs = ptrace::getregs(self.pid())?;
                Status::Stopped(signal, regs.rip as usize)
            },
            other => panic!("waited returned unexpected status: {:?}", other),
        })
    }

    /// # brief
    /// Wake up the paused inferior process, there are two possibilities:
    /// paused by breakpoints
    /// (1) inferior process paused by breakpoints
    /// (2) inferior process paused by other signals (e.g. ctrl + c)//   
    ///
    /// # param
    /// - `signal` - Optional signal to deliver to the process upon resuming execution.
    /// - `breakpoints` - A hashmap containing the addresses of breakpoints set in the process.
    ///   
    /// # return
    /// * Returns a `Result` indicating the status of the process after resuming execution. Possible
    /// return values are:
    ///
    /// * `Ok(Status::Exited(exit_code))` - If the process has exited with a specific exit code.
    /// * `Ok(Status::Signaled(signal))` - If the process has been terminated by a signal.
    /// * `Ok(Status::Stopped(signal, status))` - If the process has been stopped by a signal, with
    ///   information about the signal and the status.
    /// * `Err(nix::Error)` - If an error occurs during the execution of the function. 
    ///
    /// # Examples
    ///
    /// ```
    /// let mut debugger = Debugger::new();
    /// let breakpoints = HashMap::new();
    /// match debugger.continue_run(Some(signal::Signal::SIGCONT), &breakpoints) {
    ///     Ok(status) => {
    ///         match status {
    ///             Status::Exited(exit_code) => {
    ///                 println!("Process exited with code: {}", exit_code);
    ///             }
    ///             Status::Signaled(signal) => {
    ///                 println!("Process terminated by signal: {:?}", signal);
    ///             }
    ///             Status::Stopped(signal, status) => {
    ///                 println!("Process stopped by signal: {:?}, status: {:?}", signal, status);
    ///             }
    ///         }
    ///     }
    ///     Err(err) => {
    ///         println!("An error occurred: {:?}", err);
    ///     }
    /// }
    /// ```
    pub fn continue_run(&mut self, signal: Option<signal::Signal>, breakpoints: &HashMap<usize, u8>) -> Result<Status, nix::Error> {
        let mut regs = ptrace::getregs(self.pid())?;
        let rip = regs.rip as usize;
        // check if inferior stopped at a breakpoint
        println!("\x1b[33mbreakpoints: {:?} \n rip: {}\x1b[0m", breakpoints, rip); // Delete TOOD
        if let Some(ori_instr) = breakpoints.get(&(rip - 1)) {
            println!("stopped at a breakpoints");
            // restore the first byte of the instruction we replaced
            self.write_byte(rip - 1, *ori_instr).unwrap();
            // set %rip = %rip - 1 to rewind the instruction pointer
            regs.rip = (rip - 1) as u64;
            ptrace::setregs(self.pid(), regs).unwrap();
            // go to the next instruction
            println!("\x1b[31mExecute ptrace::step\x1b[0m"); // Delete TOOD
            ptrace::step(self.pid(), None).unwrap();
            // wait for inferior to stop due to SIGTRAP, just return if the inferior terminates here

            match self.wait(None).unwrap() {
                Status::Exited(exit_code) => return Ok(Status::Exited(exit_code)),
                Status::Signaled(signal) => return Ok(Status::Signaled(signal)),
                Status::Stopped(_, _) => {
                    // restore 0xcc in the breakpoint localtion
                    self.write_byte(rip - 1, 0xcc).unwrap();
                }
            }

        }
        println!("\x1b[32mExecute ptrace::cont\x1b[0m"); // Delete TOOD
        // resume normal execution
        ptrace::cont(self.pid(), signal)?;
        // wait for inferior to stop or terminate
        self.wait(None)
    }

    /// Executes a single step in the debugging process.
    ///
    /// # param
    /// - `breakpoints` - A reference to a `HashMap` containing the addresses of breakpoints.
    ///
    /// # return
    /// A `Result` indicating the status of the operation or an error from the `nix` library.
    ///
    pub fn step_over(
        &mut self, 
        breakpoints: &HashMap<usize, u8>, 
        step_points: &mut HashMap<usize, u8>,
        signal: Option<signal::Signal>, 
        dwarf_data: &DwarfData
    ) -> Result<Status, nix::Error> {
        let mut regs = ptrace::getregs(self.pid())?;
        let rip = regs.rip as usize;
        // check if inferior stopped at a breakpoint
        let line_object: Line = dwarf_data.get_line_from_addr(rip).unwrap();
        println!("\x1b[36mbreakpoints: {:?} \nrip: {}\x1b[0m", breakpoints, rip); // Delete TOOD
        if let Some(ori_instr) = breakpoints.get(&(rip - 1)) {
            println!("\x1b[31mstopped at a breakpoints\x1b[0m");// Delete TOOD
            // restore the first byte of the instruction we replaced
            self.write_byte(rip - 1, *ori_instr).unwrap();
            // set %rip = %rip - 1 to rewind the instruction pointer
            regs.rip = (rip - 1) as u64;
            ptrace::setregs(self.pid(), regs).unwrap();
            // go to the next instruction
            ptrace::step(self.pid(), None).unwrap();
            match self.wait(None).unwrap() {
                Status::Exited(exit_code) => return Ok(Status::Exited(exit_code)),
                Status::Signaled(signal) => return Ok(Status::Signaled(signal)),
                Status::Stopped(_, _) => {
                    // restore 0xcc in the breakpoint localtion
                    self.write_byte(rip - 1, 0xcc).unwrap();
                }
            }
        } else if let Some(ori_instr) = step_points.get(&(rip - 1)) {
            println!("\x1b[31mstopped at a breakpoints\x1b[0m");// Delete TOOD
            // restore the first byte of the instruction we replaced
            self.write_byte(rip - 1, *ori_instr).unwrap();
            // set %rip = %rip - 1 to rewind the instruction pointer
            regs.rip = (rip - 1) as u64;
            ptrace::setregs(self.pid(), regs).unwrap();
            // go to the next instruction
            ptrace::step(self.pid(), None).unwrap();
        } // else { }
        println!("\x1b[32mLine: {:?} \n\x1b[30mAddr: {:?} \nSet Line_number: {}\x1b[0m", &line_object, dwarf_data.get_addr_for_line(None, line_object.number + 1), line_object.number + 1);
        let next_addr: Option<usize> = dwarf_data.get_addr_for_line(None, line_object.number + 1);
        // exist Bug TODO
        if let Some(addr_value) = next_addr {
            println!("\x1b[32mFind the addr: {:?}\x1b[0m", addr_value); // TODO Delete
            let ori_instr = self.write_byte(addr_value, 0xcc).unwrap();
            step_points.insert(addr_value, ori_instr);
        } else { 
            println!("\x1b[32mCan't find the addr\x1b[0m"); // TODO Delete
        }

        // resume normal execution
        ptrace::cont(self.pid(), signal)?;
        // wait for inferior to stop due to SIGTRAP, just return if the inferior terminates here
        self.wait(None)
    }

    /// # brief
    /// This function uses the `ptrace` library to retrieve the register state of the current process
    /// and then loops through the function call stack, printing the source code line and 
    /// function name at each step.
    /// 
    /// # param
    /// - `debug_data` - A reference to the `DwarfData` containing the debugging information for the
    ///   current process.
    ///                                      
    ///
    /// # return
    /// A `Result` indicating success or an error from the `nix` library.
    ///
    pub fn print_backtrace(&self, debug_data: &DwarfData) -> Result<(), nix::Error> {
        let regs = ptrace::getregs(self.pid())?;
        let mut rip = regs.rip as usize;
        let mut rbp = regs.rbp as usize;

        loop {
            let _line = debug_data.get_line_from_addr(rip);
            let _func = debug_data.get_function_from_addr(rip);

            match (&_line, &_func) {
                (None, None) => println!("unknown func (source file not found)"),
                (Some(line), None) => println!("unknown func ({})", line),
                (None, Some(func)) => println!("{} (source file not found)", func),
                (Some(line), Some(func)) => println!("{} ({})", func, line),
            }

            if let Some(func) = _func {
                if func == "main" {
                    break;
                } 
            } else {
                break;
            }
            rip = ptrace::read(self.pid(), ( rbp + 8 ) as ptrace::AddressType)? as usize;
            rbp = ptrace::read(self.pid(), ( rbp     ) as ptrace::AddressType)? as usize;
        }
        Ok(())
    }

    /// # brief
    /// Writes a single byte of data to another process's memory and 
    /// returns the original byte of data at that memory address before writing.
    ///
    /// # param
    /// - `addr`: usize - memory address to write to
    /// - `val`: u8 - the byte value to write
    ///
    /// # return
    /// Returns a Result<u8, nix::Error> containing the raw bytes at this memory 
    /// address before writing, or an error object
    ///
    pub fn write_byte(&mut self, addr: usize, val: u8) -> Result<u8, nix::Error> {
        let aligned_addr = align_addr_to_word(addr);
        let byte_offset = addr - aligned_addr;
        let word = ptrace::read(self.pid(), aligned_addr as ptrace::AddressType)? as u64;

        let orig_byte = (word >> 8 * byte_offset) & 0xff;
        let masked_word = word & !(0xff << 8 * byte_offset);
        let updated_word = masked_word | ((val as u64) << 8 * byte_offset);

        ptrace::write(
          self.pid(),
          aligned_addr as ptrace::AddressType,
          updated_word as *mut std::ffi::c_void,
        )?;
        Ok(orig_byte as u8)
    }
}
