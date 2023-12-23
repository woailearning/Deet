use crate::gimli_wrapper;
use addr2line::Context;
use object::Object;
use std::convert::TryInto;
use std::{fmt, fs};

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

pub struct DwarfData {
    files: Vec<File>,
    add2line: Context<addr2line::gimli::EndianRcSlice<addr2line::gimli::RunTimeEndian>>,
}

#[derive(Clone, Debug)]
pub enum Location {
    Address(usize),
    FramePointerOffset(isize),
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            fmt::Display::fmt(self, f)
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
    pub fn 
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
    pub name: String`,
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
            memmap::Mmap::map(&files).or(Err(Error::ErrorOpeningFile))?
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

    #[allow(dead_code)]
    pub fn get_addr_for_function(&self, file: Option<&str>, func_name: &str) -> Option<usize> {
        match file {
            Some(filename) => {
                self.get_target_file(filename)?
                    .functions
                    .iter()
                    .find(|func| func.name == func_name)?
                    .address,
            },
            None => {
                for file in &self.files {
                    if let Some(func) = file.functions.iter().find(|func| func.name == func_name) 
                        return Some(func.address);
                }
            },
        }
    }

}

impl fmt::Debug for DwarfData {
    fn fmt(&self, f:&mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DwarfData {{files: {:?}}}", self.files)
    }
}

