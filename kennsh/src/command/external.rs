use std::ffi::CString;

use c_wrapper::{dl::*, types::*};

use crate::error::Error;

pub(crate) fn search_external() -> Vec<(String, String)> {

}

pub(crate) fn run_external(external_path: &str, command: &[String]) -> crate::Result<u8> {
	let lib = match DynamicLibrary::open(
		CString::new(external_path).unwrap(), 
		DLOpenKind::Lazy,
	) {
		Ok(lib) => lib,
		Err(s) => return Err(Error::DynamicLibraryError(s)),
	};
	let function = match unsafe { lib.get_symbol(CString::new(command[0]).unwrap()) } {
		Ok(f) => f,
		Err(s) => return Err(Error::DynamicLibraryError(s)),
	};
	c_wrapping_run_external(function, command)
}

fn c_wrapping_run_external(function: &mut fn(c_int, *const *const c_char) -> c_char, command: &[String]) -> crate::Result<u8> {
	let length = command.len();

	let mut argv: Vec<_> = command.into_iter().map(
		|arg| CString::new(arg as &str).unwrap()
	).map(
		|arg| arg.into_raw() as *const c_char	// Transfer ownership to C
	).collect();
}
