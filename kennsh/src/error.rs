use serde::{Serialize, Deserialize};

use c_wrapper::c_error::CError;

pub(crate) type Result<T> = core::result::Result<T, Error>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) enum Error {
	SyscallError{call_name: String, error: CError},
	RequestExit(Option<u8>),
	ParseError(crate::syntax_parser::ParseError),
	CommandNotFound(String),
	CommandPermissionDenied(String),
	FilePermissionDenied(Option<String>),
	FileNotFound(Option<String>),
	ExitCodeParseError(String),
	OtherError(String),
	ChdirRequested(String),
	EnvRemoveRequested(String),
	EnvChangeRequested(String, String),
	DynamicLibraryError(String),
	NoStatusChange,
}

impl From<crate::syntax_parser::ParseError> for Error {
    fn from(e: crate::syntax_parser::ParseError) -> Self {
        Self::ParseError(e)
    }
}

// #[macro_export]
// macro_rules! syscall {
// 	($syscall_name:expr) => ($syscall_name,());
// 	($syscall_name:expr => ($($syscall_param:expr),*)) => syscall($syscall_name => ($($syscall_param,)*); ok => ok);
// 	($syscall_name:expr; $okname:ident => $ex:expr) => (
// 		$syscall => (); $okname => $ex
// 	);
// 	($syscall_name:expr => ($($syscall_param:expr),*); $okname:ident => $ex:expr) => {
// 		match $syscall_name($($syscall_param,)*) {
// 			Ok($okname) => Ok($ex),
// 			Err(err) => Err($crate::error::Error::SyscallError {
// 				call_name: stringify!($syscall_name).to_string(),
// 				error: err,
// 			}),
// 		}
// 	};
// 	($syscall_name:expr; $okname:ident => $blk:tt) => ($syscall_name, (); $okname => $blk);
// 	($syscall_name:expr,($($syscall_param:expr),*); $okname:ident => $blk:tt) => {
// 		match $syscall_name(
// 			$(
// 				$syscall_param,
// 			)*
// 		) {
// 			Ok($okname) => Ok($blk),
// 			Err(err) => Err($crate::error::Error::SyscallError {
// 				call_name: stringify!($syscall_name).to_string(),
// 				error: err,
// 			}),
// 		}
// 	};
// 	($syscall_name:expr; match $blk:tt) => {
// 		match $syscall_name() {
// 			Ok(ok) => match ok $blk,
// 			Err(err) => Err($crate::error::Error::SyscallError {
// 				call_name: stringify!($syscall_name).to_string(),
// 				error: err,
// 			}),
// 		}
// 	};
// 	($syscall_name:expr,($($syscall_param:expr),*); match $blk:tt) => {
// 		match $syscall_name(
// 			$(
// 				$syscall_param,
// 			)*
// 		) {
// 			Ok(ok) => match ok $blk,
// 			Err(err) => Err($crate::error::Error::SyscallError {
// 				call_name: stringify!($syscall_name).to_string(),
// 				error: err,
// 			}),
// 		}
// 	}
// }
