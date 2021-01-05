pub(crate) fn set(command: &[String]) -> crate::Result<u8> {
	let command = &command[1..];
	if command.len() < 2 {
		Err(crate::Error::OtherError("\x1b[4munset\x1b[24m: Less than 2 arguments given".to_owned()))
	}
	else if command.len() == 2 {
		Err(crate::Error::EnvChangeRequested(command[0].to_string(), command[1].to_string()))
	}
	else {
		eprintln!("\x1b[4munset\x1b[24m: More than 2 arguments were supplied; all others will be ignored");
		unset(&command[..2])
	}
}

pub(crate) fn unset(command: &[String]) -> crate::Result<u8> {
	let command = &command[1..];

	if command.len() == 0 {
		Err(crate::Error::OtherError("\x1b[4munset\x1b[24m: No arguments given; expected 1 argument".to_owned()))
	}
	else if command.len() == 1 {
		Err(crate::Error::EnvRemoveRequested(command[0].to_string()))
	}
	else {
		eprintln!("\x1b[4munset\x1b[24m: More than 1 argument was supplied; all others will be ignored");
		unset(&command[..1])
	}
}
