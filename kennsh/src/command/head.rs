use std::ffi::CString;

use c_wrapper::{c_error::CError, file::{FileDescriptor, open}};
use kennsh_syscall_macro::syscall;
use skip_last_iter::SkipLastIterator;

pub(crate) fn head(command: &[String]) -> crate::Result<u8> {
	let mut mode = HeadMode::default();
	let mut count = HeadCount::default();
	let mut files: Vec<&str> = vec![];

	fn parse_option(
		mode: &mut HeadMode, 
		count: &mut HeadCount, 
		option: &str, 
		parameter: Option<&str>
	) -> Result<(), String> {
		match option {
			"-c" => {
				if let Some(parameter) = parameter {
					if let Some(parameter) = parameter.parse().ok() {
						*count = HeadCount::Bytes(parameter);
						Ok(())
					}
					else {
						Err(format!(
							"\x1b[4mhead\x1b[24m: Error while parsing parameter for -c: {}",
							parameter,
						))
					}
				}
				else {
					Err("\x1b[4mhead\x1b[24m: Expected parameter for -c".to_owned())
				}
			}
			"-n" => {
				if let Some(parameter) = parameter {
					if let Some(parameter) = parameter.parse().ok() {
						*count = HeadCount::Lines(parameter);
						Ok(())
					}
					else {
						Err(format!(
							"\x1b[4mhead\x1b[24m: Error while parsing parameter for -n: {}",
							parameter,
						))
					}
				}
				else {
					Err("\x1b[4mhead\x1b[24m: Expected parameter for -n".to_owned())
				}
			}
			"-q" => {
				if let None = parameter {
					*mode = HeadMode::Quiet;
					Ok(())
				}
				else {
					Err("\x1b[4mhead\x1b[24m: Unexpected parameter for -q".to_owned())
				}
			}
			"-v" => {
				if let None = parameter {
					*mode = HeadMode::Verbose;
					Ok(())
				}
				else {
					Err("\x1b[4mhead\x1b[24m: Unexpected parameter for -v".to_owned())
				}
			}
			other => {
				Err(format!("\x1b[4mhead\x1b[24m: Unrecognized parameter: {}", other))
			}
		}
	}

	let mut command_iter = command.iter().skip(1);
	loop {
		if let Some(item) = command_iter.next() {
			if item.starts_with("--") {
				return Err(crate::Error::OtherError(format!(
					"\x1b[4mhead\x1b[24m: Long options are not supported: {}",
					item
				)))
			}
			else if item.trim() == "-" {
				files.push(item.trim().as_ref())
			}
			else if item.starts_with('-') {
				let mut it = item.chars().skip(1);
				let param_options = ['c', 'n'];
				loop {
					if let Some(c) = it.next() {
						if param_options.contains(&c) {
							let parameter: String = it.collect();
							if parameter.is_empty() {
								parse_option(
									&mut mode, 
									&mut count, 
									&format!("-{}", c), 
									command_iter.next().map(|s| s.as_ref()),
								).map_err(|e| crate::Error::OtherError(e))?
							}
							else {
								parse_option(
									&mut mode,
									&mut count,
									&format!("-{}", c),
									Some(&parameter)
								).map_err(|e| crate::Error::OtherError(e))?
							}
							break
						}
						else {
							parse_option(
								&mut mode,
								&mut count,
								&format!("-{}", c),
								None,		
							).map_err(|e| crate::Error::OtherError(e))?
						}
					}
					else {
						break
					}
				}
			}
			else {
				files.push(item.trim().as_ref())
			}
		}
		else {
			break
		}
	}

	files.retain(|e| !e.trim().is_empty());
	if files.is_empty() {
		files.push("-");
	}

	if let HeadMode::Auto = mode {
		mode = if files.len() > 1 {
			HeadMode::Verbose
		}
		else {
			HeadMode::Quiet
		}
	}

	for file in files {
		let source = if file == "-" {
			syscall!(
				FileDescriptor::try_clone_stdin()
			)?
		}
		else { 
			syscall!(
				open::open_with_flags(CString::new(file.to_string()).unwrap(), open::flags::O_RDONLY);
				match_error {
					CError::NotFound => crate::Error::FileNotFound(Some(file.to_owned())),
					CError::PermissionDenied => crate::Error::FilePermissionDenied(Some(file.to_owned())),
				}
			)?
		};

		let file = if file == "-" {
			"standard input"
		}
		else {
			file
		};

		head_print(
			source,
			file,
			&mode,
			&count,
		).map_err(|e| crate::Error::OtherError(e))?
	}

	Ok(0)
}

enum HeadMode {
	Auto,
	Verbose,
	Quiet,
}

impl Default for HeadMode {
    fn default() -> Self {
        Self::Auto
    }
}

enum HeadCount {
	Bytes(i64),
	Lines(i32),
}

impl Default for HeadCount {
    fn default() -> Self {
        Self::Lines(10)
    }
}

fn head_print<R: std::io::Read>(source: R, filename: &str, mode: &HeadMode, count: &HeadCount) -> Result<(), String> {
	let source = std::io::BufReader::new(source);
	
	if let HeadMode::Verbose = mode {
		println!("==> {} <==", filename);
	}

	match count {
	    HeadCount::Bytes(count) => {
			let it = std::io::Read::bytes(source);
			let it: Box<dyn Iterator<Item = std::io::Result<u8>>> = if count < &0 {
				Box::from(SkipLastIterator::new(it, (-count) as usize))
			}
			else {
				Box::from(it.take(*count as usize))
			};
			let stdout = std::io::stdout();
			let mut stdout = stdout.lock();
			use std::io::Write;
			for byte in it.into_iter() {
				match byte {
					Ok(s) => {
						stdout.write(&[s]).unwrap();
						stdout.flush().unwrap();
						// eprintln!("Iteration!");
						// print!("{}", char::from(s));
					},
					Err(e) => {
						return Err(format!(
							"\x1b[4mhead\x1b[24m: IO Error occured while reading from file {}: {:?}",
							filename,
							e,
						))
					}
				}
			}
		}
	    HeadCount::Lines(count) => {
			let it = std::io::BufRead::lines(source);
			let it: Box<dyn Iterator<Item = std::io::Result<String>>> = if count < &0 {
				Box::from(SkipLastIterator::new(it, (-count) as usize))
			}
			else {
				Box::from(it.take(*count as usize))
			};
			for line in it.into_iter() {
				match line {
					Ok(s) => {
						println!("{}", s);
					},
					Err(e) => {
						return Err(format!(
							"\x1b[4mhead\x1b[24m: IO Error occured while reading from file {}: {:?}",
							filename,
							e,
						))
					}
				}
			}
		}
	};

	if let HeadMode::Verbose = mode {
		println!();
	}

	Ok(())
}
