use std::iter::once;

use c_wrapper::{c_error::CError, file::FileDescriptor};
use kennsh_syscall_macro::syscall;

pub(crate) fn cat(command: &[String]) -> crate::Result<u8> {
	let mut options = CatOptions::default();
	for item in command.iter().skip(1) {
		if item.starts_with('-') {
			for c in item[1..].chars() {
				options
					.parse_option(&("-".to_owned() + &c.to_string()))
					.map_err(|e| crate::Error::OtherError(e))?
			}
		}
		else if item.starts_with('=') {
			for c in item[1..].chars() {
				options
					.parse_option(&("=".to_owned() + &c.to_string()))
					.map_err(|e| crate::Error::OtherError(e))?
			}
		}
		else {
			cat_print(item, &options)?
		}
	}
	Ok(0)
}

fn cat_print(filename: &str, options: &CatOptions) -> crate::Result<()> {
	let reader: FileDescriptor = match filename {
		"-" => syscall!(FileDescriptor::try_clone_stdin)?,
		filename => syscall!(c_wrapper::file::open::open_with_flags(
			std::ffi::CString::new(filename).unwrap(),
			c_wrapper::file::open::flags::O_RDONLY,
		); match_error {
			CError::NotFound => crate::Error::FileNotFound(Some(filename.to_owned())),
			CError::PermissionDenied => crate::Error::FilePermissionDenied(Some(filename.to_owned())),
		})?,
	};

	// if options.no_buffering {
	// 	cat_print_unbuffered(
	// 		filename,
	// 		reader,
	// 		syscall!(FileDescriptor::try_clone_stdout)?,
	// 		options,
	// 	)
	// }
	// else {
		cat_print_buffered(
			filename,
			reader,
			syscall!(FileDescriptor::try_clone_stdout)?,
			options,
		)
	// }

	// let mut is_line_empty = true;
	// let mut empty_lines_read = 0;
	// let mut lines_read = 0;
	// let mut line_start = true;

	// loop {
	// 	match std::io::Read::read(&mut reader, &mut buffer) {
	// 		Ok(0) => break Ok(()),
	// 		Ok(bytes) => {
	// 			for i in 0..bytes {
	// 				let mut b = buffer[i];
	// 				if b > 127 {
	// 					output.write_all("buf")
	// 					b -= 128;
	// 				}
	// 			}
	// 		}
	// 		Err(_) => break Err(crate::Error::OtherError(
	// 			format!("\x1b[4mcat\x1b[24m: Unknown error while reading file: {}", filename)
	// 		))
	// 	}
	// }
}

// fn cat_print_unbuffered<R: std::io::Read, W: std::io::Write>(filename: &str, input: R, output: W, options: &CatOptions) -> crate::Result<()> {
// 	let mut buffer = [0];
// 	loop {
// 		match input.read(&mut buffer) {
// 			Ok(0) => break Ok(()),
// 			Ok(1) => {

// 			},
// 			Ok(_) => panic!("Impossible!"),
// 			Err(_) => break Err(crate::Error::OtherError(
// 				format!("\x1b[4mcat\x1b[24m: Error while reading from file: {}", filename)
// 			))
// 		}
// 	}
// }

fn cat_print_buffered<R: std::io::Read, W: std::io::Write>(filename: &str, input: R, output: W, options: &CatOptions) -> crate::Result<()> {
	let input = std::io::BufReader::new(input);
	let mut output = std::io::BufWriter::new(output);

	use std::io::Write;

	let mut current_line = 0;
	let mut previous_line_empty = false;

	for line in std::io::BufRead::lines(input) {
		match line {
			Ok(line) => {
				let is_line_empty = line.trim().is_empty();
				
				if options.no_repeated_empty_lines {
					if is_line_empty {
						if !previous_line_empty {
							previous_line_empty = true;
						}
						else {
							continue;
						}
					}
					else {
						previous_line_empty = false;
					}
				}

				if options.number_lines {
					if !options.number_nonempty_lines || !is_line_empty {
						current_line += 1;
						write!(output, "{:6}  ", current_line).map_err(|e| {
							crate::Error::OtherError(format!(
								"\x1b[4mcat\x1b[24m: Unknown IO Error: {:?}",
								e,
							))
						})?;
					}
				}

				let line: String = if options.show_non_printable {
					line.bytes()
        				.flat_map(|b| {
							if b > 0x7F {
								either::Either::Left("M-".bytes().chain(once(b - 0x7F)))
							}
							else {
								either::Either::Right(once(b))
							}
						})
        				.flat_map(|b| {
							use either::Either::*;
							match b {
								0o000 => Left("^".bytes().chain(once('@' as u8))),
								0o177 => Left("^".bytes().chain(once('?' as u8))),
								b if b == '\n' as u8 => {
									Right(once(b))
								}
								b if b == '\t' as u8 => {
									if options.show_tabs {
										Left("^".bytes().chain(once(
											'I' as u8
										)))
									}
									else {
										Right(once(b))
									}
								}
								b if b < 0o040 => {
									Left("^".bytes().chain(once(
										('A' as u8) - 1 + b
									)))
								} 
								b => Right(once(b))
							}
						})
						.map(char::from)
						.collect()
				}
				else {
					line.chars()
        				.flat_map(|c| {
							use either::Either::*;
							if c == '\t' {
								Left("^I".chars())
							}
							else {
								Right(once(c))
							}
						})
        				.collect()
				};
				write!(output, "{}", line).map_err(|e| {
					crate::Error::OtherError(format!(
						"\x1b[4mcat\x1b[24m: Unknown IO Error: {:?}",
						e,
					))
				})?;

				if options.dollar_at_end_of_line {
					write!(output, "$").map_err(|e| {
						crate::Error::OtherError(format!(
							"\x1b[4mcat\x1b[24m: Unknown IO Error: {:?}",
							e,
						))
					})?;
				}

				writeln!(output).map_err(|e| {
					crate::Error::OtherError(format!(
						"\x1b[4mcat\x1b[24m: Unknown IO Error: {:?}",
						e,
					))
				})?;
			}
			Err(e) => {
				return Err(crate::Error::OtherError(format!(
					"\x1b[4mcat\x1b[24m: IO Error occured while reading from file {}: {:?}",
					filename,
					e,
				)))
			}
		}
	}

	Ok(())
}

struct CatOptions {
	pub dollar_at_end_of_line: bool,
	pub number_nonempty_lines: bool,
	pub number_lines: bool,
	pub no_repeated_empty_lines: bool,
	pub show_tabs: bool,
	pub show_non_printable: bool,
	pub no_buffering: bool,			// Incompatible with number_non*
}

impl Default for CatOptions {
    fn default() -> Self {
        Self {
			dollar_at_end_of_line: false,
			number_lines: false,
			number_nonempty_lines: false,
			no_repeated_empty_lines: false,
			show_tabs: false,
			show_non_printable: false,
			no_buffering: false,
		}
    }
}

impl CatOptions {
	fn parse_option(&mut self, option: &str) -> Result<(), String> {
		match option.trim() {
			"-A" => {
				self.parse_option("-v")?;
				self.parse_option("-E")?;
				self.parse_option("-T")?;
				Ok(())
			},
			"-b" => {
				self.parse_option("-n")?;
				self.number_nonempty_lines = true;
				if self.no_buffering {
					Err(
						format!("\x1b[4mcat\x1b[24m: Options -b and -u cannot be set together")
					)
				}
				else {
					Ok(())
				}
			},
			"-e" => {
				self.parse_option("-v")?;
				self.parse_option("-E")?;
				Ok(())
			}
			"-E" => {
				self.dollar_at_end_of_line = true;
				Ok(())
			}
			"-n" => {
				self.number_lines = true;
				Ok(())
			}
			"-s" => {
				self.no_repeated_empty_lines = true;
				Ok(())
			}
			"-t" => {
				self.parse_option("-v")?;
				self.parse_option("-T")?;
				Ok(())
			}
			"-T" => {
				self.show_tabs = true;
				Ok(())
			}
			"-u" => {
				self.no_buffering = true;
				if self.number_nonempty_lines {
					Err(
						format!("\x1b[4mcat\x1b[24m: Options -b and -u cannot be set together")
					)
				}
				else {
					Ok(())
				}
			}
			"-v" => {
				self.show_non_printable = true;
				Ok(())
			}
			"=A" => {
				self.parse_option("=v")?;
				self.parse_option("=E")?;
				self.parse_option("=T")?;
				Ok(())
			},
			"=b" => {
				self.number_lines = false;
				self.number_nonempty_lines = false;
				Ok(())
			},
			"=e" => {
				self.parse_option("=v")?;
				self.parse_option("=E")?;
				Ok(())
			}
			"=E" => {
				self.dollar_at_end_of_line = false;
				Ok(())
			}
			"=n" => {
				self.number_lines = false;
				Ok(())
			}
			"=s" => {
				self.no_repeated_empty_lines = false;
				Ok(())
			}
			"=t" => {
				self.parse_option("=v")?;
				self.parse_option("=T")?;
				Ok(())
			}
			"=T" => {
				self.show_tabs = false;
				Ok(())
			}
			"=u" => {
				self.no_buffering = false;
				Ok(())
			}
			"=v" => {
				self.show_non_printable = false;
				Ok(())
			}
			other => Err(format!("\x1b[4mcat\x1b[24m: Unrecognized option: {}", other)),
		}
	}
}
