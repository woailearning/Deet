use addr2line::Context;
use object::Object;
use std::convert::TryInto;
use std::{fmt, fs};

use crate::gimli_wrapper;

#[derive(Debug)]
pub enum Error {
    ErrorOpeningFile,
    DwarfFormatError(gimli_wrapper::Error),
}

impl From<gimli_wrapper::Error> for Error {
    fn from(err: gimli_wrapper::Error) -> Self {
        Error::DwarfFormatError(err)
    }
}

#[derive(Clone)]
pub enum Location {
    Address(usize),
    FramePointerOffset(isize),
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Location::Address(addr) => write!(f, "Address({:#x})", addr),
            Location::FramePointerOffset(offset) => write!(f, "FramePointerOffset({})", offset),
        }
    }
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Type {
    pub name: String,
    pub size: usize,
}

impl Type {
    pub fn new(name: String, size: usize) -> Self {
        Type {name, size,}
    }
}

// For variables and formal parameters
#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub entity_type: Type,
    pub location: Location,
    pub line_number: usize, // Line number in source file
}

#[derive(Debug, Default, Clone)]
pub struct Function {
    pub name: String,
    pub address: usize,
    pub text_length: usize,
    pub line_number: usize,
    pub variables: Vec<Variable>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub file: String,
    pub number: usize,
    pub address: usize,
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file, self.number)
    }
}

#[derive(Debug, Default, Clone)]
pub struct File {
    pub name: String,
    pub global_variables: Vec<Variable>,
    pub functions: Vec<Function>,
    pub lines: Vec<Line>,
}

pub struct DwarfData {
    files: Vec<File>,
    addr2line: Context<addr2line::gimli::EndianRcSlice<addr2line::gimli::RunTimeEndian>>,
}

impl DwarfData {

    /// # Brief
    ///
    /// Create a `DwarfData` object from a file.
    ///
    /// This function opens the file specified by the `path` parameter and creates a `DwarfData` object
    /// containing the parsed debug information from the file.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the file to be opened and parsed.
    ///
    /// # Returns
    ///
    /// * `Result<DwarfData, Error>` - A `Result` indicating success (`Ok`) with the created `DwarfData` object,
    ///   or an error (`Err`) if there was a problem opening the file or parsing the debug information.
    ///
    pub fn from_file(path: &str) -> Result<Self, Error> {
        let file = fs::File::open(path).or(Err(Error::ErrorOpeningFile))?;
        let mmap = unsafe { 
            memmap::Mmap::map(&file).or(Err(Error::ErrorOpeningFile))?
        };
        let object = object::File::parse(&*mmap)
            .or_else(|e| Err(gimli_wrapper::Error::ObjectError(e.to_string())))?;
        let endian = if object.is_little_endian() {
            gimli::RunTimeEndian::Little
        } else {
            gimli::RunTimeEndian::Big
        };
        Ok(DwarfData {
            files: gimli_wrapper::load_file(&object, endian)?,
            addr2line: Context::new(&object).or_else(|e| Err(gimli_wrapper::Error::from(e)))?,
        })
    }

    /// # Brief
    ///
    /// Find the target file in the list of files.
    ///
    /// This function searches for a file in the list of files based on the given `file` parameter.
    /// It checks if the file name matches exactly with `file`, or if `file` does not contain a slash '/'
    /// and the file name ends with `file` preceded by a slash '/'.
    ///
    /// # Arguments
    ///
    /// * `file` - A string representing the target file name.
    ///
    /// # Returns
    ///
    /// An optional reference to the target `File` if found, or `None` if not found.
    ///
    #[allow(dead_code)]
    fn get_target_file(&self, file: &str) -> Option<&File> {
        self.files.iter().find(|f| {
            (f.name == file) || (!file.contains("/") && f.name.ends_with(&format!("/{}", file)))
        })
    }

    /// Retrieves the memory address corresponding to a specified file and line number.
    /// 
    /// # Param
    /// 
    /// * `file`: Optional filename. If `None`, the first file is selected by default.
    /// * `line_number`: The line number in the source code.
    /// 
    /// # Returns
    /// 
    /// If the corresponding line is found, the memory address of that line is returned. Otherwise, `None` is returned.
    #[allow(dead_code)]
    pub fn get_addr_for_line(&self, file: Option<&str>, line_number: usize) -> Option<usize> {
        let target_file = match file {
            Some(filename) => self.get_target_file(filename)?,
            None => self.files.get(0)?,
        };
        Some(
            target_file
                .lines
                .iter()
                .find(|line| line.number >= line_number)?
                .address,
            )
    }

    /// 
    /// Retrieves the memory address corresponding to a specified file and function name.
    /// 
    /// # Param
    /// 
    /// * `file`: Optional filename. If `None`, the function is searched for in all files.
    /// * `func_name`: The name of the function.
    /// 
    /// # Returns
    /// 
    /// If the corresponding function is found, the memory address of that function is returned. Otherwise, `None` is returned.
    #[allow(dead_code)]
    pub fn get_addr_for_function(&self, file: Option<&str>, func_name: &str) -> Option<usize> {
        match file {
            Some(filename) => Some(
                self.get_target_file(filename)?
                    .functions
                    .iter()
                    .find(|func| func.name == func_name)?
                    .address,
            ),
            None => {
                for file in &self.files {
                    if let Some(func) = file.functions.iter().find(|func| func.name == func_name) {
                        return Some(func.address);
                    }
                }
                None
            },
        }
    }

    /// Retrieves the source code line information corresponding to a memory address.
    /// 
    /// # Param
    /// 
    /// * `curr_addr`: The memory address.
    /// 
    /// # Returns
    /// 
    /// If the corresponding source code line is found, the information of that line is returned. Otherwise, `None` is returned.
    #[allow(dead_code)]
    pub fn get_line_from_addr(&self, curr_addr: usize) -> Option<Line> {
        let location = self
            .addr2line
            .find_location(curr_addr.try_into().unwrap())
            .ok()??;
        Some( Line{
            file: location.file?.to_string(),
            number: location.line?.try_into().unwrap(),
            address: curr_addr,
        })
    }

    /// Retrieves the function name corresponding to a memory address.
    /// 
    /// # Parameters
    /// 
    /// * `curr_addr`: The memory address.
    /// 
    /// # Returns
    /// 
    /// If the corresponding function is found, the name of that function is returned. Otherwise, `None` is returned.
    #[allow(dead_code)]
    pub fn get_function_from_addr(&self, curr_addr: usize) -> Option<String> {
        let frame = self
            .addr2line
            .find_frames(curr_addr.try_into().unwrap())
            .ok()?
            .next()
            .ok()??;
        Some( frame.function?.raw_name().ok()?.to_string() )
    }

    /// Prints the details of the DWARF data.
    ///
    /// This function iterates over each file in the DWARF data and prints its name, global variables, functions, and line numbers.
    /// For each global variable and function variable, it prints the name, type, location, and line number.
    /// For each function, it prints the name, declaration line number, memory address, and text length.
    /// For each line number, it prints the line number and its corresponding memory address.
    ///
    /// # Example Output
    ///
    /// ```
    /// ------
    /// filename.rs
    /// ------
    /// Global variables:
    ///    * Variable: var1 (type1, located at location1, line_number1 bytes long)
    /// Functions:
    ///    * func1 (declared on line line_number2, located at 0xaddress1, text_length1 bytes long)
    ///    * Variable: var2 (type2, located at location2, line_number3 bytes long)
    /// Line numbers:
    ///    * line_number4 (at 0xaddress2)
    /// ```plaintext
    ///
    /// # Note
    ///
    /// This function is primarily used for debugging and understanding the structure of the DWARF data.
    #[allow(dead_code)]
    pub fn print(&self) {
        for file in &self.files {
            println!("------");
            println!("{}", file.name);
            println!("------");

        println!("\x1b[34m| - - - - Global variables- - - - : |\x1b[0m");
            for var in &file.global_variables {
                println!(
                    "| Variable: {:<20} | Type: {:<8} | Location: {:<10} | Line: {:<5} |",
                    var.name, var.entity_type.name, var.location, var.line_number
                );
            }

            println!("\x1b[34m| - - - - Functions- - - - : |\x1b[0m");
            for func in &file.functions {
                println!(
                    "| Function: {:<17} | Line: {:<8} | Address: {:<24x} | Length: {:<6} |",
                    func.name, func.line_number, func.address, func.text_length,
                );
                for var in &func.variables {
                    println!(
                    "| Variable: {:<17} | Type: {:<8} | Location: {:<20} | Line: {:<8} |",
                        var.name, var.entity_type.name, var.location, var.line_number
                    );
                }
            }

            println!("\x1b[34m| - - - - Line numbers- - - - : |\x1b[0m");
            for line in &file.lines {
                println!(
                    "| Line: {:<5} | Address: {:<5x} |",
                    line.number, line.address
                );
            }
        }
    }
}

impl fmt::Debug for DwarfData {
    fn fmt(&self, f:&mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DwarfData {{files: {:?}}}", self.files)
    }
}

