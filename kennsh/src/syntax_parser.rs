use std::fmt::Display;
use serde::{Deserialize, Serialize};

use crate::str_extension::StringExtensions;

static ESCAPE_CHAR: char = '`';
static PIPE_CHAR: char = '|';
static STRING_CHARS: [char; 2] = ['"', '\''];

pub(crate) fn parse(input: String) -> Result<SyntaxTree, ParseError> {
	parse_pipe_chain(0, &input)
}

fn parse_pipe_chain(_start_index: usize, input: &str) -> Result<SyntaxTree, ParseError> {
	let mut _starting_from = 0;
	let mut pipe_char_vec = vec![];

	let mut skip_next = false;	// If backtick, skip the next char, as it is escaped
	let mut skip_char = None;
	for (index, c) in input.as_bytes().iter().map(|c| char::from(*c)).enumerate() {
		if skip_next {
			skip_next = false;
		}
		else if let Some(sc) = skip_char {
			if c == sc {
				skip_char = None;
			}
		}
		else if c == ESCAPE_CHAR {
			skip_next = true;
		}
		// Skip pipe character if part of the >| sequence
		else if c == '>' {
			skip_next = true;
		}
		else if STRING_CHARS.contains(&c) {
			skip_char = Some(c);
		}
		else if c == PIPE_CHAR {
			pipe_char_vec.push(index);
		}
	};

	if pipe_char_vec.is_empty() {
		parse_command(0, input).map(|cs| {
			SyntaxTree::Command(cs)
		})
	}
	else {
		let mut length = 0;
		let mut syntaxes = vec![];
		for command in input.split_at_multiple(&pipe_char_vec) {
			let trimmed = command.trim();
			let start_index = length + command.chars().take_while(|c| c.is_whitespace()).collect::<Vec<_>>().len();

			let syntax = parse_command(start_index, trimmed)?;
			syntaxes.push(syntax);

			length += command.len() + 1;
		}
		Ok(SyntaxTree::PipeChain(syntaxes))
	}
}

fn parse_command(start_index: usize, input: &str) -> Result<CommandSyntax, ParseError> {
	if input.is_empty() {
		return Err(ParseError {
			start_index,
			end_index: start_index + 1,
			reason: "An empty command was given".to_owned(),
		});
	}

	let splitted = {
		let mut result = vec![];
		let mut starting_index = 0;
		let mut buffer = String::new();
		let mut skip_next_char = false;
		let mut skip_char: Option<char> = None;
		for (index, c) in input.bytes().into_iter().map(|c| char::from(c)).enumerate() {
			if skip_next_char {
				skip_next_char = false;
				buffer += &c.to_string();
			}
			else if let Some(sc) = skip_char {
				if sc == c || (sc == '(' && c == ')') {
					skip_char = None;
				}
				buffer += &c.to_string();
			}
			else if c == ESCAPE_CHAR {
				skip_next_char = true;
				buffer += &c.to_string();
			}
			else if STRING_CHARS.contains(&c) || c == '(' {
				// Incorporate strings and subcommands
				skip_char = Some(c);
				buffer += &c.to_string();
			}
			else if c == ' ' {
				// Separate words by space
				result.push((starting_index, buffer));
				buffer = String::new();
				starting_index = index + 1;
			}
			else {
				buffer += &c.to_string();
			}
		}
		if !buffer.is_empty() {
			result.push((starting_index, buffer));
		}
		result
	};

	let mut command_parts = vec![];
	let mut command_syntax = CommandSyntax::Command(vec![]);
	// Process input and stuff
	let mut iter = splitted.into_iter();
	'item_loop: while let Some((index, item)) = iter.next() {
		let mut skip_next_char = false;
		let mut skip_char = None;
		let utf8_item = item
			.as_bytes()
			.into_iter()
    		.map(|u| char::from(*u))
			.collect::<Vec<_>>();
		for (i, c) in utf8_item.into_iter().enumerate() {
			if skip_next_char {
				skip_next_char = false
			}
			else if c == ESCAPE_CHAR {
				skip_next_char = true
			}
			else if let Some(sc) = skip_char {
				if c == sc {
					skip_char = None;
				}
			}
			else if STRING_CHARS.contains(&c) {
				skip_char = Some(c);
			}
			else if c == '<' {
				let fd_part = &item[..i];
				let file_part = &item[i+1..];
				let fd = match fd_part {
					"" => None,
					"0" => Some(0),
					_ => return Err(ParseError {
						start_index: index,
						end_index: index + i,
						reason: "Only 0 or the default file descriptor (empty) are supported".to_owned(), 
					}),
				};
				// Check if filename is in the next item
				let filename = if file_part.is_empty() {
					if let Some(x) = iter.next() {
						x
					}
					else {
						return Err(ParseError {
							start_index: index + i + 1,
							end_index: index + i + 2,
							reason: "No file for redirection was given".to_owned(),
						})
					}
				} else { (i+1, file_part.to_owned()) };
				// Check if filename is a file descriptor
				// In case of input piping, don't accept any file descriptor yet
				// TODO: Implement file descriptor management
				if filename.1.chars().nth(0).unwrap() == '&' {
					return Err(ParseError {
						start_index: filename.0,
						end_index: filename.0 + filename.1.len(),
						reason: "Only files are accepted at the moment (perhaps escape & with `?)".to_owned(),
					})
				}
				// Change the syntax to an input redirection one
				command_syntax = CommandSyntax::InputRedirection {
					command: Box::from(command_syntax),
					file_descriptor: fd,
					filename: filename.1.to_owned(),
				};
				continue 'item_loop;
			}
			else if c == '>' {
				let fd_part = &item[..i];
				// eprintln!("fd_part: {}; len {}", fd_part, fd_part.len());
				let file_part = item[i+1..].to_owned();
				let mut file_part = file_part.trim();
				// eprintln!("file_part: {}; len {}", file_part, file_part.len());
				let fd = match fd_part {
					"" => None,
					"1" => Some(1),
					"2" => Some(2),
					_ => return Err(ParseError {
						start_index: index,
						end_index: index + i,
						reason: "Only 1, 2 or the default file descriptor (empty) are supported".to_owned(), 
					}),
				};
				// Check the kind of the redirection
				let kind = if file_part.is_empty() {
					OutputRedirectionKind::Create
				}
				else {
					match file_part.chars().nth(0).unwrap() {
						'|' => {
							file_part = &file_part[1..];
							OutputRedirectionKind::Overwrite
						},
						'>' => {
							file_part = &file_part[1..];
							OutputRedirectionKind::Append
						},
						_ => OutputRedirectionKind::Create,
					}
				};
				// Check if filename is in the next item
				let filename = if file_part.is_empty() {
					if let Some(x) = iter.next() {
						// eprintln!("iter.next(): {}; len {}", x.1, x.1.len());
						(x.0, x.1.to_owned())
					}
					else {
						return Err(ParseError {
							start_index: index + i + 1,
							end_index: index + i + 2,
							reason: "No file for redirection was given".to_owned(),
						})
					}
				} else { (i+1, file_part.to_owned()) };
				// Check if redirecting into another file descriptor
				// In case of output piping, accept only piping from 1 to 2
				// or from 2 to 1
				// TODO: Implement file descriptor management
				if filename.1.chars().nth(0).unwrap() == '&' {
					match filename.1[1..].parse::<i32>() {
						Ok(1) | Ok(2) => {},
						Ok(_) => {
							return Err(ParseError {
								start_index: filename.0,
								end_index: filename.0 + filename.1.len(),
								reason: "For now, only file descriptors 1 (stdout) and 2 (stderr) are supported".to_owned(),			
							});
						},
						Err(_) => {
							return Err(ParseError {
								start_index: filename.0 + 1,
								end_index: filename.0 + filename.1.len(),
								reason: "Could not convert into an integer file descriptor".to_owned(),
							});
						}
					}
				}
				
				command_syntax = CommandSyntax::OutputRedirection {
					command: Box::from(command_syntax),
					file_descriptor: fd,
					destination: filename.1.to_owned(),
					kind,
				};
				continue 'item_loop;
			}
		}
		command_parts.push(parse_command_item(index, &item)?);
	}
	command_syntax = command_syntax.inject_and_replace(command_parts);
	Ok(command_syntax)
}

fn parse_command_item(start_index: usize, input: &str) -> Result<CommandItem, ParseError> {
	let mut result = vec![];

	// asdab"test"$status(echo meow)
	// |    |     |      ^ subcommand
	// |    |     ^ environment variable
	// |    ^ string
	// ^ raw string

	enum CurrentlyFilling {
		RawString,
		String(char),
		Subcommand,
		ShellVariable,
	}

	let mut currently_filling = CurrentlyFilling::RawString;
	let mut subcommand_recursivity_count = 0;
	let mut buffer = String::new();

	let char_iter = input
		.as_bytes()
		.into_iter()
		.map(|b| char::from(*b));

	let mut next_char_escaped = false;

	for (i, c) in char_iter.enumerate() {
		if next_char_escaped {
			let new_c = match c {
				'n' => '\n',
				'r' => '\r',
				't' => '\t',
				c => c,
			};
			buffer += &new_c.to_string();
			next_char_escaped = false;
		}
		else if c == ESCAPE_CHAR {
			next_char_escaped = true;
		}
		else if let CurrentlyFilling::Subcommand = currently_filling {
			if c == ')' {
				subcommand_recursivity_count -= 1;
				if subcommand_recursivity_count == 0 {
					result.push(
						CommandItem::Subcommand(
							Box::new(
								parse_command(start_index + i - buffer.len(), &buffer)?
							)
						)
					);
					buffer = String::new();
					currently_filling = CurrentlyFilling::RawString;
				}
				else {
					buffer += &c.to_string();
				}
			}
			else if c == '(' {
				subcommand_recursivity_count += 1;
				buffer += &c.to_string();
			}
			else {
				buffer += &c.to_string();
			}
		}
		else if STRING_CHARS.contains(&c) {
			if let CurrentlyFilling::String(sc) = currently_filling {
				if c == sc {
					result.push(CommandItem::String(buffer));
					buffer = String::new();
					currently_filling = CurrentlyFilling::RawString;
				}
				else {
					buffer += &c.to_string();
				}
			}
			else {
				// No string started, start a new one
				match currently_filling {
					CurrentlyFilling::RawString => {
						result.push(CommandItem::RawString(buffer));
						buffer = String::new();
					}
					CurrentlyFilling::String(_) => {},
					CurrentlyFilling::ShellVariable => {
						result.push(CommandItem::ShellVariable(buffer));
						buffer = String::new();
					},
					CurrentlyFilling::Subcommand => {
						return Err(ParseError {
							start_index: start_index + i,
							end_index: start_index + i + i,
							reason: "Found \" without closing parenthesis from the subcommand".to_owned(),
						});
					}
				};
				currently_filling = CurrentlyFilling::String(c);
			}
		}
		else if let CurrentlyFilling::String(_) = currently_filling {
			buffer += &c.to_string();
		}
		else if c == '(' {
			subcommand_recursivity_count += 1;
			match currently_filling {
				CurrentlyFilling::Subcommand => {
					return Err(ParseError {
						start_index: start_index + i,
						end_index: start_index + i + 1,
						reason: "Subcommands within subcommands are not yet supported".to_owned(),
					})
				},
				CurrentlyFilling::String(_) => {
					panic!("syntax_parser.rs, parse_command_item, Impossible");
				},
				CurrentlyFilling::RawString => {
					result.push(CommandItem::RawString(buffer));
					buffer = String::new();
					currently_filling = CurrentlyFilling::Subcommand;
				},
				CurrentlyFilling::ShellVariable => {
					result.push(CommandItem::ShellVariable(buffer));
					buffer = String::new();
					currently_filling = CurrentlyFilling::Subcommand;
				}
			}
		}
		else if c == ')' {
			if let CurrentlyFilling::Subcommand = currently_filling {
				result.push(
					CommandItem::Subcommand(
						Box::new(
							parse_command(start_index + i - buffer.len(), &buffer)?
						)
					)
				);
				buffer = String::new();
				currently_filling = CurrentlyFilling::RawString;
			}
			else {
				return Err(ParseError {
					start_index: start_index + i,
					end_index: start_index + i + i,
					reason: "Found closing parenthesis without a previous open parenthesis".to_owned(),
				});
			}
		}
		else if c == '$' {
			match currently_filling {
				CurrentlyFilling::Subcommand => {
					return Err(ParseError {
						start_index: start_index + i,
						end_index: start_index + i + 1,
						reason: "Subcommands within subcommands are not yet supported".to_owned(),
					})
				},
				CurrentlyFilling::String(_) => {
					panic!("syntax_parser.rs, parse_command_item, Impossible");
				},
				CurrentlyFilling::RawString => {
					result.push(CommandItem::RawString(buffer));
					buffer = String::new();
					currently_filling = CurrentlyFilling::ShellVariable;
				},
				CurrentlyFilling::ShellVariable => {
					result.push(CommandItem::ShellVariable(buffer));
					buffer = String::new();
					currently_filling = CurrentlyFilling::ShellVariable;
				}
			}
		}
		else {
			buffer += &c.to_string();
		}
	}

	if !buffer.is_empty() {
		match currently_filling {
			CurrentlyFilling::RawString => result.push(CommandItem::RawString(buffer)),
			CurrentlyFilling::String(_) => result.push(CommandItem::String(buffer)),
			CurrentlyFilling::ShellVariable => result.push(CommandItem::ShellVariable(buffer)),
			CurrentlyFilling::Subcommand => {
				return Err(ParseError {
					start_index: input.len(),
					end_index: input.len(),
					reason: "Found end of string instead of ) while reading subcommand".to_owned(),
				})
			}
		}
	}

	Ok(CommandItem::Combination(result).normalize())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct ParseError {
	pub start_index: usize,
	pub end_index: usize,
	pub reason: String,
}

#[derive(Clone, Debug)]
pub(crate) enum SyntaxTree {
	Command(CommandSyntax),
	PipeChain(Vec<CommandSyntax>),
}

impl Display for SyntaxTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyntaxTree::Command(cmd) => write!(f, "{}", cmd),
            SyntaxTree::PipeChain(chain) => {
				let mut chain: Vec<String> = chain.iter().flat_map(|elem| {
					vec![elem.to_string(), " | ".to_string()]
				}).collect();
				chain.remove(chain.len() - 1);
				let chain = chain.iter().fold("".to_string(), |a, e| {
					a + e
				});
				write!(f, "{}", chain)
			}
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum CommandSyntax {
	InputRedirection {
		command: Box<CommandSyntax>,
		// This will always be None for now
		// TODO: Standalone file descriptor support => remove comment
		file_descriptor: Option<i32>,
		filename: String
	},
	OutputRedirection {
		command: Box<CommandSyntax>,
		file_descriptor: Option<i32>,
		destination: String,
		kind: OutputRedirectionKind,
	},
	Command(Vec<CommandItem>),
}

impl CommandSyntax {
	fn inject_and_replace(self, new_command: Vec<CommandItem>) -> Self {
		match self {
		    Self::InputRedirection { command, file_descriptor, filename } => {
				Self::InputRedirection {
					command: Box::from(command.inject_and_replace(new_command)),
					file_descriptor,
					filename,
				}
			}
		    Self::OutputRedirection { command, file_descriptor, destination, kind } => {
				Self::OutputRedirection {
					command: Box::from(command.inject_and_replace(new_command)),
					file_descriptor,
					destination,
					kind,
				}
			}
		    Self::Command(_old_command) => Self::Command(new_command),
		}
	}
}

impl Display for CommandSyntax {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandSyntax::InputRedirection { command, file_descriptor, filename } => {
				write!(f, "{} {}< {}", *command, file_descriptor.map(|fd| fd.to_string()).unwrap_or("".to_owned()), filename)
			}
            CommandSyntax::OutputRedirection { command, file_descriptor, destination, kind } => {
				write!(f, "{} ", *command)?;
				if let Some(fd) = file_descriptor {
					write!(f, "{}", fd)?;
				}
				write!(f, "{}", kind)?;
				write!(f, "{}", destination)
			}
            CommandSyntax::Command(cmd) => {
				let mut cmd: Vec<_> = cmd.into_iter().flat_map(|ci| {
					vec![ci.to_string(), " ".to_string()]
				}).collect();
				cmd.remove(cmd.len() - 1);
				let cmd = cmd.into_iter().fold("".to_string(), |a, s| {
					a + &s
				});
				write!(f, "{}", cmd)
			}
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum OutputRedirectionKind {
	Create,
	Append,
	Overwrite,
}

impl Default for OutputRedirectionKind {
    fn default() -> Self {
		Self::Create
    }
}

impl Display for OutputRedirectionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputRedirectionKind::Create => write!(f, ">"),
            OutputRedirectionKind::Append => write!(f, ">>"),
            OutputRedirectionKind::Overwrite => write!(f, ">|"),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum CommandItem {
	String(String),
	RawString(String),
	ShellVariable(String),
	Subcommand(Box<CommandSyntax>),
	Combination(Vec<CommandItem>),
}

impl CommandItem {
	pub(crate) fn flatten(self) -> Self {
		match self {
			Self::Combination(items) => {
				let mut new_items = vec![];
				for item in items {
					match item {
						Self::Combination(mut inner_items) => {
							new_items.append(&mut inner_items)
						}
						Self::RawString(s) => {
							if !s.is_empty() {
								new_items.push(Self::RawString(s))
							}
						}
						_ => new_items.push(item),
					}
				}
				for item in &mut new_items {
					*item = item.clone().flatten();
				}
				if new_items.len() == 1 {
					new_items.remove(0)
				}
				else {
					Self::Combination(new_items)
				}
			}
			_ => self,
		}
	}

	pub(crate) fn normalize(mut self) -> Self {
		self = self.flatten();
		match self {
			CommandItem::Combination(mut c) if c.len() == 1 => c.remove(0),
			_ => self,
		}
	}
}

impl Display for CommandItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
			CommandItem::String(s) => write!(f, "\"{}\"", s),
            CommandItem::ShellVariable(var_name) => {
				write!(f, "${}", var_name)
			},
            CommandItem::Combination(items) => {
				for item in items {
					write!(f, "{}", item)?;
				}
				Ok(())
			}
            CommandItem::Subcommand(s) => write!(f, "({})", s),
            CommandItem::RawString(s) => write!(f, "{}", s),
        }
    }
}
