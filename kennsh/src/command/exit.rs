use crate::error::Error;

pub(crate) fn exit_command(command: &[String]) -> crate::Result<u8> {
	let exit_code = match command.len() {
		1 => None,
		2 => match command[1].parse() {
		    Ok(exit_code) => Some(exit_code),
		    Err(_) => {
				return Err(Error::ExitCodeParseError(command[1].to_string()))
			}
		},
		_ => {
			eprintln!("[Warning] More than 1 argument was supplied to exit; only the 1st argument will be used");
			Some(exit_command(&command[0..2])?)
		}
	};
	Err(Error::RequestExit(exit_code))
}
