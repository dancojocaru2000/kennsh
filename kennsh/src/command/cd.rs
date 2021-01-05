use crate::error::Error;

pub(crate) fn cd(command: &[String]) -> crate::Result<u8> {
	match command.len() {
		1 => Err(Error::OtherError("cd requires 1 parameter with the path to change into".to_owned())),
		2 => {
			let mut path = command[1].clone();
			if let Some(index) = path.find('~') {
				path.replace_range(index..=index, &std::env::var("HOME").unwrap());
			}
			Err(Error::ChdirRequested(path))
			// syscall!(chdir(CString::new(path).unwrap()))?;
			// Ok(0)
		},
		_ => {
			eprintln!("[Warning] More than 1 argument was supplied to \x1b[4mcd\x1b[0m; only the 1st argument will be used");
			cd(&command[0..2])
		}
	}
}

