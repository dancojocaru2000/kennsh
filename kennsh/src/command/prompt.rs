pub(crate) fn prompt() -> crate::Result<u8> {
	print!("{}", crate::prompt(std::env::var("status").map_err(|_| ()).and_then(|v| v.parse().map_err(|_| ())).unwrap_or(0) != 0));
	Err(crate::Error::NoStatusChange)
}
