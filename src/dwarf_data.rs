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

pub struct DwarfData {
    files: Vec<File>,
    add2line: Context<addr2line::gimli::EndianRcSlice<addr2line::gimli::RunTimeEndian>>,
}

impl fmt::Debug for DwarfData {
    fn fmt(&self, f:&mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "DwarfData {{files: {:?}}}", self.files)
    }
}

impl From<gimli_wrapper::Error> for Error {
    fn from(err: gimli_wrapper::Error) -> Self {
        Error::DwarfFormatError(err)
    }
}

impl DwarfData {
    ///
    pub fn from_file(path: &str}) -> Result<DwarfData, Error> {
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

    ///
    #[allow(dead_code)]
    fn get_target_file(&self, file: &str) -> Option<&File> {
        self.files.iter().find(|f| {
            f.name == file || (!file.contains("/") && f.name.ends_with(&format!("/{}", file)))
        })
    }

    #[allow(dead_code)]
    pub fn get_addr_for_line(&self, file: Option<&str>, line_number: usize) -> Option<usize> {
    }
}
