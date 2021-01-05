//mod external;

mod cat;
mod cd;
use cd::cd;
mod color_test;
use color_test::color_test;
mod env;
mod exit;
use exit::exit_command;
mod head;
mod prompt;
mod server;
mod set;

use std::{ffi::CString, io::{Read, Write}, mem, process::exit};

use c_wrapper::{c_error::CError, chdir::chdir, exec, file, file::{FileDescriptor, open}, fork::{ForkResult, fork}, pipe::pipe, wait};
use file::access::{AccessCheck, access};
use kennsh_syscall_macro::syscall;
use syntax_parser::CommandSyntax;

use crate::{env_util::env_is_true, error::Error, syntax_parser::{self, CommandItem, parse}, window_title::{WindowTitleElement, set_window_title}};

pub(crate) fn handle_subcommand<'a, Bytes: AsRef<[u8]>>(subcommand: String, stdin: Option<Bytes>) -> crate::Result<(u8, FileDescriptor)> {
	// Create pipe for stdin of subcommand
	let stdin_pipe = if let Some(_) = stdin {
		Some(syscall!(pipe)?)
	}
	else { None };
	let stdout_pipe = syscall!(pipe)?;

	// Fork and handle
	syscall!(fork match {
		ForkResult::Child => {
			std::env::set_var("no_title", "yes");
			if let Some(stdin_pipe) = stdin_pipe {
				let stdin_read = stdin_pipe.drop_write();
				FileDescriptor::wrap_stdin(|stdin| {
					syscall!(FileDescriptor::redirect_from(stdin, &stdin_read))
				})?
			}
			let stdout_write = stdout_pipe.drop_read();
			FileDescriptor::wrap_stdout(|stdout| {
				syscall!(FileDescriptor::redirect_from(stdout, &stdout_write))
			})?;

			// In subcommand, don't re-print syntax tree
			std::env::remove_var("print_syntax_tree");

			exit(handle(subcommand).unwrap().into())
		},
		ForkResult::Parent(child_pid) => {
			if let Some(stdin) = stdin {
				let stdin = stdin.as_ref();
				let mut stdin_write = stdin_pipe.unwrap().drop_read();
				let mut last_index = 0;
				while last_index < stdin.len() {
					let bytes_written = syscall!(FileDescriptor::write_slice(&mut stdin_write, &stdin[last_index..]))?;
					last_index += bytes_written;
				}
			}
			let stdout_read = stdout_pipe.drop_write();

			let exit_code = syscall!(wait::waitpid(child_pid); it.status.exit_status())?;

			(exit_code, stdout_read)
		}
	})
}

pub(crate) fn handle(command: String) -> crate::Result<u8> {
	if !env_is_true("no_title") {
		set_window_title(vec![
			WindowTitleElement::CustomText(command.split(' ').nth(0).unwrap().to_owned()),
			WindowTitleElement::Separator,
			WindowTitleElement::ShortCurrentWorkingDirectory,
		]);
	}
	// Create subprocess to execute the command into
	let result_pipe = syscall!(pipe)?;
	syscall!(fork match {
		ForkResult::Child => {
			let mut result_write = result_pipe.drop_read();

			let parsed_command = parse(command);
			let result = match parsed_command {
				Ok(tree) => {
					handle_tree(tree)
				}
				Err(e) => {
					Err(Error::ParseError(e))
				}
			};

			
			// syscall!(FileDescriptor::write_any(&mut result_write, result))?;
			let _ = Write::write_all(&mut result_write, serde_json::to_string(&result).unwrap().as_bytes());
			exit(0);
		},
		ForkResult::Parent(child_pid) => {
			let mut result_read = result_pipe.drop_write();
			// This is extremely unsafe and extremely bodge-y but I hope it works
			// let result = unsafe { syscall!(FileDescriptor::read_any(&mut result_read))? };
			let mut s = String::new();
			let _ = result_read.read_to_string(&mut s);
			let result = serde_json::from_str(&s).unwrap();
			syscall!(wait::waitpid(child_pid))?;
			match result {
				Err(Error::ChdirRequested(path)) => {
					syscall!(chdir(CString::new(path.clone()).unwrap()); match_error {
						CError::NotFound => {
							return Err(Error::FileNotFound(Some(path)))
						}
					})?;
					let _ = syscall!(
						c_wrapper::cwd::getcwd; 
						std::env::set_var("PWD", it)
					);
					Ok(0)
				},
				Err(Error::EnvChangeRequested(key, value)) => {
					std::env::set_var(key, value);
					Ok(0)
				},
				Err(Error::EnvRemoveRequested(key)) => {
					std::env::remove_var(key);
					Ok(0)
				}
				any => any,
			}
		}
	})?
}

fn handle_tree(command: syntax_parser::SyntaxTree) -> crate::Result<u8> {
	static INDENT_STR: &str = "    ";
	let print_tree = env_is_true("print_syntax_tree");
	if print_tree {
		eprintln!("\x1b[4m$print_syntax_tree\x1b[24m:");
		fn ci_print(item: &CommandItem, indent: usize) {
			match item {
			    CommandItem::String(s) => {
					eprintln!("{}String: {}", INDENT_STR.repeat(indent), s);
				}
			    CommandItem::RawString(rs) => {
					eprintln!("{}Raw String: {}", INDENT_STR.repeat(indent), rs);
				}
			    CommandItem::ShellVariable(var) => {
					eprintln!("{}Shell Variable: {}", INDENT_STR.repeat(indent), var);
				}
			    CommandItem::Subcommand(sc) => {
					eprintln!("{}Subcommand", INDENT_STR.repeat(indent));
					cs_print(sc, indent + 1);
				}
			    CommandItem::Combination(cmb) => {
					for (index, item) in cmb.iter().enumerate() {
						eprintln!("{}Combination item {}", INDENT_STR.repeat(indent), index + 1);
						ci_print(item, indent + 1);
					}
				}
			}
		}
		fn cs_print(command: &CommandSyntax, indent: usize) {
			match command {
			    CommandSyntax::InputRedirection { command, file_descriptor, filename } => {
					eprintln!("{}Input redirection", INDENT_STR.repeat(indent));
					eprintln!(
						"{}File descriptor: {}", 
						INDENT_STR.repeat(indent + 1), 
						file_descriptor.map_or("Default".to_owned(), |fd| fd.to_string()),
					);
					eprintln!(
						"{}Redirect from: {}", 
						INDENT_STR.repeat(indent + 1), 
						filename,
					);
					cs_print(command, indent + 1);
				}
			    CommandSyntax::OutputRedirection { command, file_descriptor, destination, kind } => {
					eprintln!("{}Output redirection", INDENT_STR.repeat(indent));
					eprintln!(
						"{}File descriptor: {}", 
						INDENT_STR.repeat(indent + 1), 
						file_descriptor.map_or("Default".to_owned(), |fd| fd.to_string()),
					);
					eprintln!(
						"{}Redirect into: {}", 
						INDENT_STR.repeat(indent + 1), 
						destination,
					);
					eprintln!(
						"{}Redirect kind: {:2} {}", 
						INDENT_STR.repeat(indent + 1), 
						kind,
						match kind {
							syntax_parser::OutputRedirectionKind::Append => "Append",
							syntax_parser::OutputRedirectionKind::Create => "Create",
							syntax_parser::OutputRedirectionKind::Overwrite => "Overwrite",
						},
					);
					cs_print(command, indent + 1);
				}
			    CommandSyntax::Command(cmd) => {
					for (index, item) in cmd.iter().enumerate() {
						eprintln!("{}Command item {}", INDENT_STR.repeat(indent), index + 1);
						ci_print(item, indent + 1);
					}
				}
			}
		}
		fn st_print(tree: &syntax_parser::SyntaxTree, indent: usize) {
			match tree {
			    syntax_parser::SyntaxTree::Command(cs) => {
					eprintln!("{}Command", INDENT_STR.repeat(indent));
					cs_print(cs, indent + 1);
				}
			    syntax_parser::SyntaxTree::PipeChain(chain) => {
					for (index, cs) in chain.iter().enumerate() {
						eprintln!("{}Pipe chain - Command {}", INDENT_STR.repeat(indent), index + 1);
						cs_print(cs, indent + 1);
					}
				}
			}
		}

		st_print(&command, 0);
	}

	match command {
	    syntax_parser::SyntaxTree::Command(c) => handle_command(c),
	    syntax_parser::SyntaxTree::PipeChain(chain) => handle_pipe(chain),
	}
}

fn handle_pipe(commands: Vec<CommandSyntax>) -> crate::Result<u8> {
	let mut old_pipe_read = None;
	for (index, command) in commands.iter().enumerate() {
		let new_pipe = syscall!(pipe)?;
		syscall!(fork match {
			ForkResult::Child => {
				// The child will write to the new pipe
				// The next child will read from it as it will become the old pipe
				let mut new_pipe_write = new_pipe.drop_read();
				if let Some(mut old_pipe_read) = old_pipe_read {
					FileDescriptor::wrap_stdin(|stdin| {
						syscall!(FileDescriptor::redirect_from(stdin, &mut old_pipe_read))
					})?
				}
				if index != commands.len() - 1 {
					FileDescriptor::wrap_stdout(|stdout| {
						syscall!(FileDescriptor::redirect_from(stdout, &mut new_pipe_write))
					})?
				}
				else {
					// If this is the last command, close the writing pipe
					// as we are instead writing to stdout directly
					mem::drop(new_pipe_write)
				}
				// Execute command
				// Unwrap the result so that an error is printed.
				// Since the child process is in another process entirely from
				// the parent process, there's no read way to transmit the error to the 
				// parent, so at least have it visible.
				let cmd_result = handle_command(command.clone()).unwrap();
				// Exit with the exit call of the child
				// This is only relevant for the last command in the chain, which will
				// be waited for by the main process, and whose exit code will be 
				// returned
				exit(cmd_result.into())
			},
			ForkResult::Parent(child_pid) => {
				// Setup the new pipe to be the old pipe of the future process
				old_pipe_read = Some(new_pipe.drop_write());
				// If this is the last process, wait for it and return its status
				if index == commands.len() - 1 {
					return syscall!(wait::waitpid(child_pid); res => res.status.exit_status())
				}
			}
		})?
	};
	panic!("Reached end of pipe loop without returning the status of last command")
}

fn handle_command(command: syntax_parser::CommandSyntax) -> crate::Result<u8> {
	match command {
	    CommandSyntax::InputRedirection { command, file_descriptor, filename } => {
			// Open the given file, possibly returning error
			let file = syscall!(
				open::open_with_flags(CString::new(filename.clone()).unwrap(), file::open::flags::O_RDONLY); 
				match_error {
					CError::NotFound => crate::Error::FileNotFound(Some(filename)),
				}
			)?;
			// Redirect stdin by default
			let file_descriptor = file_descriptor.unwrap_or(file::constants::STDIN_FILENO);
			// Redirect the file descriptor to the file
			FileDescriptor::wrap_unowned(file_descriptor, |fd| {
				syscall!(FileDescriptor::redirect_from(fd, &file))
			})?;
			// Redirect stdin to the file
			// FileDescriptor::wrap_stdin(|stdin| {
			// 	syscall!(FileDescriptor::redirect_from(stdin, &file))
			// })?;
			// Run command
			handle_command(*command)
		}
	    CommandSyntax::OutputRedirection { command, file_descriptor, destination, kind } => {
			// Open the given file, possibly returning error
			let file_flags = file::open::flags::O_WRONLY | match kind {
			    syntax_parser::OutputRedirectionKind::Create => {
					// Check if file exists
					if syscall!(access(CString::new(destination.clone()).unwrap(), AccessCheck::FileExists))? {
						return Err(
							Error::OtherError(
								format!("The file {} already exists. Use >| to overwrite the file or >> to append to it.", destination)
							)
						);
					}
					file::open::flags::O_CREAT
				}
			    syntax_parser::OutputRedirectionKind::Append => {
					file::open::flags::O_APPEND | file::open::flags::O_CREAT
				}
			    syntax_parser::OutputRedirectionKind::Overwrite => {
					file::open::flags::O_TRUNC
				}
			};
			let file = syscall!(
				open::open_with_mode(CString::new(destination.clone()).unwrap(), file_flags, 0o777); 
				match_error {
					CError::NotFound => crate::Error::FileNotFound(Some(destination)),
					CError::PermissionDenied => crate::Error::FilePermissionDenied(Some(destination)),
				}
			)?;
			// Redirect stdout by default
			let file_descriptor = file_descriptor.unwrap_or(file::constants::STDOUT_FILENO);
			// Redirect the file descriptor to the file
			FileDescriptor::wrap_unowned(file_descriptor, |fd| {
				syscall!(FileDescriptor::redirect_from(fd, &file))
			})?;
			// Run command
			handle_command(*command)
		}
	    CommandSyntax::Command(command_items) => {
			let command_items: crate::Result<Vec<String>> =
				command_items
				.into_iter()
				.map(|ci| {
					evaluate_command_item(ci)
				})
        		.collect();
			execute_command(&command_items?)
		}
	}
}

fn evaluate_command_item(command_item: CommandItem) -> crate::Result<String> {
	evaluate_command_item_2(command_item, false)
}

fn evaluate_command_item_2(command_item: CommandItem, raw_as_normal_str: bool) -> crate::Result<String> {
	match command_item {
	    CommandItem::String(s) => Ok(s),
	    CommandItem::ShellVariable(var_name) => {
			Ok(std::env::var(var_name).unwrap_or("".to_string()))
		}
	    CommandItem::Subcommand(sc) => {
			// Make pipe, run sc, return stdout of sc
			let (_, mut read_pipe) = handle_subcommand(sc.to_string(), None::<Vec<u8>>)?;
			let mut res = String::new();
			let _ = read_pipe.read_to_string(&mut res);
			Ok(res.trim().to_owned())
		}
	    CommandItem::Combination(items) => {
			let mut result = String::new();

			for item in items {
				result += &evaluate_command_item_2(item, true)?;
			}

			Ok(result)
		}
	    CommandItem::RawString(rs) => {
			// TODO: Is RawString actually necessary?
			// 2020-12-31 21:26  Okay, seriously, why the heck did I do this? XD
			if raw_as_normal_str {
				Ok(rs)
			}
			else {
				Ok(rs)
			}
		}
	}
}

fn execute_command(command: &[String]) -> crate::Result<u8> {
	let command_executable = command[0].clone();

	let who_is_running = env_is_true("who_is_running");

	if command_executable == "exit" {
		if who_is_running {
			eprintln!("\x1b[4m$who_is_running\x1b[24m: Internal command: exit")
		}
		exit_command(&command)
	}
	else if command_executable == "~server" {
		if who_is_running {
			eprintln!("\x1b[4m$who_is_running\x1b[24m: Internal command: ~server")
		}
		server::server(&command)
	}
	else if command_executable == "~color_test" {
		if who_is_running {
			eprintln!("\x1b[4m$who_is_running\x1b[24m: Internal command: ~color_test")
		}
		color_test()
	}
	else if command_executable == "~prompt" {
		if who_is_running {
			eprintln!("\x1b[4m$who_is_running\x1b[24m: Internal command: ~prompt")
		}
		prompt::prompt()
	}
	else if command_executable == "~set" {
		if who_is_running {
			eprintln!("\x1b[4m$who_is_running\x1b[24m: Internal command: ~set")
		}
		set::set(&command)
	}
	else if command_executable == "~unset" {
		if who_is_running {
			eprintln!("\x1b[4m$who_is_running\x1b[24m: Internal command: ~unset")
		}
		set::unset(&command)
	}
	else if command_executable == "cd" {
		if who_is_running {
			eprintln!("\x1b[4m$who_is_running\x1b[24m: Internal command: cd")
		}
		cd(&command)
	}
	else if command_executable == "env" {
		if who_is_running {
			eprintln!("\x1b[4m$who_is_running\x1b[24m: Internal command: env")
		}
		env::env(&command)
	}
	else if command_executable == "head" {
		if who_is_running {
			eprintln!("\x1b[4m$who_is_running\x1b[24m: Internal command: head")
		}
		head::head(&command)
	}
	else if command_executable == "cat" {
		if who_is_running {
			eprintln!("\x1b[4m$who_is_running\x1b[24m: Internal command: cat")
		}
		cat::cat(&command)
	}
	// else if command detection
	else {
		if who_is_running {
			eprintln!("\x1b[4m$who_is_running\x1b[24m: External command")
		}
		handle_extern(&command)
	}

}

// fn redirect_then_handle(command: String) -> crate::Result<u8> {
// 	let redirect_split = command.split('>').collect::<Vec<&str>>();
// 	let append = if redirect_split.len() == 1 {
// 		return handle(command);
// 	} else if redirect_split.len() == 2 {
// 		false
// 	} else if redirect_split.len() == 3 {
// 		// TODO: Check that it's not a > b > c, but only a >> c
// 		true
// 	} else {
// 		// TODO: Handle the error gracefully
// 		return Err(Error::RedirectSyntaxError);
// 	};
// 	let command = redirect_split[0];
// 	let filename = redirect_split[1];
// }

// fn pipe_then_handle(command: String) -> crate::Result<u8> {
// 	handle(command)
// }
//
// fn handle(command: String) -> crate::Result<u8> {
// 	let argv: Vec<String> = command.split(' ').map(|elem| elem.to_string()).collect();
// 	// TODO: Dynamically check for internal commands and such
// 	if argv[0] == "exit" {
// 		return Err(Error::RequestExit(0));
// 	}
// 	else if argv[0] == "~color_test" {
// 		color_test()
// 	}
// 	else {
// 		handle_extern(argv)
// 	}
// }

fn handle_extern(command: &[String]) -> crate::Result<u8> {
	// match fork() {
	// 	Ok(ForkResult::Child) => {
	// 		match exec::execp(&command[0], command.clone()) {
	// 			Ok(()) => {}
	// 			Err(err) => panic!(&format!("Error on execp: {}", err))
	// 		};
	// 		exit(1)
	// 	}
	// 	Ok(ForkResult::Parent(child_pid)) => {
	// 		// match wait::waitpid(child_pid) {
	// 		// 	Ok(res) => Ok(res.status.exit_status()),
	// 		// 	Err(err) => Err(Error::SyscallError {
	// 		// 		call_name: "waitpid",
	// 		// 		error: err,
	// 		// 	}),
	// 		// };

	// 		syscall!(wait::waitpid, (child_pid); res => {
	// 			res.status.exit_status()
	// 		})
	// 	},
	//     Err(err) => Err(Error::SyscallError {
	// 		call_name: "fork",
	// 		error: err,
	// 	}),
	// };

	let command: Vec<_> = command.into_iter().map(|s| s.trim().to_owned()).collect();
	let command = &command;

	syscall!(fork match no_wrap {
		ForkResult::Child => {
			if env_is_true("who_is_running_ext") {
				eprintln!(" idx │ c │ dec │ hex ");
				eprintln!("━━━━━┿━━━┿━━━━━┿━━━━━");
				for (i, b) in command[0].clone().as_bytes().iter().enumerate() {
					let c = char::from(*b);
					eprintln!(
						" {0:3} │ {1} │ {2:3} │ {2:3X} ", 
						i, 
						if c.is_control() {
							' '
						}
						else {
							c
						}, 
						b,
					)
				}
			}

			syscall!(
				exec::execp(&command[0], command);
				match_error {
					CError::NotFound => exit(127),
					CError::PermissionDenied => exit(126),
				}
			).expect("[Error] execp returned an error");
			exit(1)
		},
		ForkResult::Parent(child_pid) => {
			match syscall!(wait::waitpid(child_pid); it.status.exit_status()) {
				Ok(127) => return Err(Error::CommandNotFound(command[0].clone())),
				Ok(126) => return Err(Error::CommandPermissionDenied(command[0].clone())),
				any => any,
			}
		},
	})
}
