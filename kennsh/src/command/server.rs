use std::net::{Ipv4Addr, SocketAddrV4};
use c_wrapper::{file::FileDescriptor, fork::{ForkResult, fork}, pipe::pipe};
use socket2::*;
use uuid::Uuid;
use crate::env_util::env_is_true;

pub(crate) fn server(command: &[String]) -> crate::Result<u8> {
	let socket_debug = env_is_true("socket_debug");

	let port = if command.len() > 2 {
		eprintln!("\x1b[4mserver\x1b[24m: More than 1 argument was supplied; all others will be ignored");
		return server(&command[..2])
	}
	else if command.len() == 2 {
		command[1].parse().map_err(|e| {
			crate::Error::OtherError(format!(
				"\x1b[4mserver\x1b[24m: Error when parsing the port: {:?}",
				e
			))
		})?
	}
	else {
		7500u16
	};

	let socket = Socket::new(
		Domain::ipv4(), 
		Type::stream(), 
		None
	).map_err(|e| {
		crate::Error::OtherError(format!(
			"\x1b[4mserver\x1b[24m: Unknown error while creating socket: {:?}",
			e,	
		))
	})?;
	if socket_debug {
		eprintln!("\x1b[4mserver\x1b[24m: Created socket");
	}

	socket.bind(
		&SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), port).into()
	).map_err(|e| crate::Error::OtherError(format!(
		"\x1b[4mserver\x1b[24m: Unknown error while binding socket: {:?}",
		e,
	)))?;
	if socket_debug {
		eprintln!("\x1b[4mserver\x1b[24m: Bound socket to port {}", port);
	}

	socket.listen(500).map_err(|e| crate::Error::OtherError(format!(
		"\x1b[4mserver\x1b[24m: Unknown error while binding socket: {:?}",
		e,
	)))?;
	if socket_debug {
		eprintln!("\x1b[4mserver\x1b[24m: Started listening on socket");
	}
	else {
		println!("\x1b[4mserver\x1b[24m: Server listening on port {}", port);
	}

	loop {
		match socket.accept() {
			Ok((socket, address)) => {
				let client = Client::new(socket);
				println!("\x1b[4mserver\x1b[24m: {}: Client connected from address {:?}", client.uuid, address);
				handle_client(client)
			}
			Err(e) => {
				eprintln!("\x1b[4mserver\x1b[24m: Unknown error while accepting connection: {:?}", e);
			}
		}
	}
}

use std::io::{Read, Write};

fn handle_client(mut client: Client) {
	let socket_debug = env_is_true("socket_debug");
	if socket_debug {
		eprintln!("\x1b[4mserver\x1b[24m: {}: Starting thread", client.uuid);
	}
	std::thread::spawn(move || {
		let stdin_pipe = pipe().unwrap();
		let stdout_pipe = pipe().unwrap();
		let stderr_pipe = pipe().unwrap();

		// Spawn child shell to handle server
		if let ForkResult::Child = fork().unwrap() {
			// Do redirects
			let new_stdin = stdin_pipe.drop_write();
			let new_stdout = stdout_pipe.drop_read();
			let new_stderr = stderr_pipe.drop_read();
			FileDescriptor::wrap_stdin(|stdin| {
				stdin.redirect_from(&new_stdin)
			}).unwrap();
			FileDescriptor::wrap_stdout(|stdout| {
				stdout.redirect_from(&new_stdout)
			}).unwrap();
			FileDescriptor::wrap_stderr(|stderr| {
				stderr.redirect_from(&new_stderr)
			}).unwrap();

			// Start process
			crate::main();
			if socket_debug {
				eprintln!("\x1b[4mserver\x1b[24m: {}: Child shell ended", client.uuid);
			}
			std::process::exit(0);
		}
		if socket_debug {
			eprintln!("\x1b[4mserver\x1b[24m: {}: Spawned child shell", client.uuid);
		}

		let mut stdin_pipe = stdin_pipe.drop_read();
		let mut stdout_pipe = stdout_pipe.drop_write();
		let mut stderr_pipe = stderr_pipe.drop_write();

		loop {
			// If client has message for shell
			let client_message = {
				client.socket.set_nonblocking(true).unwrap();
				let mut length = [0; 4];
				let length = match client.socket.read_exact(&mut length) {
					Ok(_) => Some(u32::from_be_bytes(length)),
					Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => None,
					Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
						// Client disconnected
						break
					},
					Err(e) => Err(e).unwrap(),
				};
				if let Some(length) = length {
					client.socket.set_nonblocking(false).unwrap();
					let mut buffer = vec![0; length as usize];
					match client.socket.read_exact(&mut buffer) {
						Ok(()) => {},
						Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
							// Client disconnected
							break
						},
						Err(e) => {
							Err(e).unwrap()
						}
					}
					Some(buffer)
				}
				else { None }
			};
			if let Some(client_message) = client_message {
				if socket_debug {
					eprintln!("\x1b[4mserver\x1b[24m: {}: Get message \"{}\" from client", client.uuid, String::from_utf8_lossy(&client_message));
				}
				if client_message.len() > 0 {
					match stdin_pipe.write_all(&client_message) {
						Err(ref e) if e.kind() == std::io::ErrorKind::WriteZero => {
							// Child shell ended
							if socket_debug {
								eprintln!("\x1b[4mserver\x1b[24m: {}: Child shell ended", client.uuid);
							}
							let _ = client.socket.shutdown(std::net::Shutdown::Both);
							break;
						},
						any => any.unwrap(),
					}
				}
			} 

			// If shell send stdout
			let stdout_message = {
				stdout_pipe.set_nonblocking(true).unwrap();
				let mut buffer = [0; 4096];
				match stdout_pipe.read(&mut buffer) {
					Ok(0) => {
						// Child shell ended
						if socket_debug {
							eprintln!("\x1b[4mserver\x1b[24m: {}: Child shell ended", client.uuid);
						}
						let _ = client.socket.shutdown(std::net::Shutdown::Both);
						break
					},
					Ok(bytes) => Some(buffer[..bytes].to_vec()),
					Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => None,
					Err(e) => {
						Err(e).unwrap()
					}
				}
			};
			if let Some(stdout_message) = stdout_message {
				if socket_debug {
					eprintln!("\x1b[4mserver\x1b[24m: {}: Sending stdout message: \"{}\"", client.uuid, String::from_utf8_lossy(&stdout_message));
				}
				if stdout_message.len() > 0 {
					match client.socket.write_all(&1u32.to_be_bytes()) {
						Err(ref e) if e.kind() == std::io::ErrorKind::WriteZero => {
							// Client disconnected
							break;
						},
						any => any.unwrap(),
					}
					match client.socket.write_all(&(stdout_message.len() as u32).to_be_bytes()) {
						Err(ref e) if e.kind() == std::io::ErrorKind::WriteZero => {
							// Client disconnected
							break;
						},
						any => any.unwrap(),
					}
					match client.socket.write_all(&stdout_message) {
						Err(ref e) if e.kind() == std::io::ErrorKind::WriteZero => {
							// Client disconnected
							break;
						},
						any => any.unwrap(),
					}
				}
			}

			// If shell send stderr
			let stderr_message = {
				stderr_pipe.set_nonblocking(true).unwrap();
				let mut buffer = [0; 4096];
				match stderr_pipe.read(&mut buffer) {
					Ok(0) => {
						// Child shell ended
						if socket_debug {
							eprintln!("\x1b[4mserver\x1b[24m: {}: Child shell ended", client.uuid);
						}
						let _ = client.socket.shutdown(std::net::Shutdown::Both);
						break
					},
					Ok(bytes) => Some(buffer[..bytes].to_vec()),
					Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => None,
					Err(e) => {
						Err(e).unwrap()
					}
				}
			};
			if let Some(stderr_message) = stderr_message {
				if socket_debug {
					eprintln!("\x1b[4mserver\x1b[24m: {}: Sending stderr message: \"{}\"", client.uuid, String::from_utf8_lossy(&stderr_message));
				}
				if stderr_message.len() > 0 {
					match client.socket.write_all(&2u32.to_be_bytes()) {
						Err(ref e) if e.kind() == std::io::ErrorKind::WriteZero => {
							// Client disconnected
							break;
						},
						any => any.unwrap(),
					}
					match client.socket.write_all(&(stderr_message.len() as u32).to_be_bytes()) {
						Err(ref e) if e.kind() == std::io::ErrorKind::WriteZero => {
							// Client disconnected
							break;
						},
						any => any.unwrap(),
					}
					match client.socket.write_all(&stderr_message) {
						Err(ref e) if e.kind() == std::io::ErrorKind::WriteZero => {
							// Client disconnected
							break;
						},
						any => any.unwrap(),
					}
				}
			}
		}

		println!("\x1b[4mserver\x1b[24m: {} disconnected", client.uuid);
	});
}

struct Client {
	pub socket: Socket,
	pub uuid: Uuid,
}

impl Client {
	pub fn new(socket: Socket) -> Self {
		Self {
			socket,
			uuid: Uuid::new_v4(),
		}
	}
}

impl PartialEq for Client {
    fn eq(&self, other: &Self) -> bool {
        self.uuid.eq(&other.uuid)
    }
}
impl Eq for Client {

}

impl core::hash::Hash for Client {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.uuid.hash(state)
    }
}
