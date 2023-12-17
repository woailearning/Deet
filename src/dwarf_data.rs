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
