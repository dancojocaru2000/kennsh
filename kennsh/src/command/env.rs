use c_wrapper::{file::FileDescriptor, fork::{ForkResult::*, fork}, pipe::pipe, wait};
use kennsh_syscall_macro::syscall;
use peek_iter::PeekIterator;

pub(crate) fn env(command: &[String]) -> crate::Result<u8> {
	if command.len() == 1 {
		// Print environment if no argument supplied
		for (key, value) in std::env::vars() {
			let formatted_string = if should_print_color() {
				format!("{2}{0}{4}={3}{1}{4}", key, value, "\x1b[94m", "\x1b[92m", "\x1b[39m")
			}
			else {
				format!("{0}={1}", key, value)
			};
			println!("{}", formatted_string);
		}
		Ok(0)
	}
	else {
		let pipe = syscall!(pipe)?;

		syscall!(fork match no_wrap {
			Child => {
				let mut it: PeekIterator<&String, _> = PeekIterator::from(command[1..].iter());

				fn opt_i() {
					// Clear environment
					for (key, _) in std::env::vars_os() {
						std::env::remove_var(key);
					}
				}

				fn opt_u<S: AsRef<std::ffi::OsStr>>(key: S) {
					std::env::remove_var(key);
				}

				let rest_command_iterator = loop {
					let item = it.peek();
					if let Some(item) = item {
						let item = item.trim();
						if item == "-i" {
							opt_i();
							it.next();
						}
						else if item.starts_with("-u") {
							it.next();
							let key = if item.len() > 2 {
								&item[2..]
							}	
							else {
								match it.next() {
									Some(item) => item,
									None => {
										return Err(
											crate::Error::OtherError(
												"\x1b[4menv\x1b[24m: Found end of parameters after -u; expected key to remove".to_owned()
											)
										);
									}
								}
							};
							opt_u(key);
						}
						else if item.starts_with('-') {
							return Err(crate::Error::OtherError(
								format!("\x1b[4menv\x1b[24m: Unrecognized option: {}", item)
							))
						}
						else if item.contains('=') {
							it.next();
							let parts: Vec<_> = item.split('=').collect();
							if parts.len() > 2 {
								return Err(
									crate::Error::OtherError(
										format!("\x1b[4menv\x1b[0m: more than 1 equal sign (=) found in the following parameter: {}", item).to_owned()
									)
								);
							}
							std::env::set_var(parts[0], parts[1]);
						}
						else {
							break Some(it)
						}
					}
					else {
						break None
					}
				};
				let command = match rest_command_iterator {
					Some(it) => it.map(|i| i.to_owned()).collect(),
					None => vec!["env".to_owned()],
				};

				let mut result_write = pipe.drop_read();
				let result = super::execute_command(&command);
				let result_str = serde_json::to_string(&result).unwrap();
				let _ = std::io::Write::write_all(
					&mut result_write,
					result_str.as_bytes()
				);

				std::process::exit(0);
			},
			Parent(child_pid) => {
				let mut result_read = pipe.drop_write();
				let mut s = String::new();
				let _ = std::io::Read::read_to_string(&mut result_read, &mut s);

				let result = serde_json::from_str(&s).unwrap();
				syscall!(wait::waitpid(child_pid))?;

				result
			}
		})
	}
}

fn should_print_color() -> bool {
	!(std::env::var("NOCOLOR").map_or(false, |_| true) ||
	std::env::var("NO_COLOR").map_or(false, |_| true) ||
	FileDescriptor::wrap_stdout(|stdout| {
		!stdout.is_a_tty()
	}))
}
