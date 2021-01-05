pub fn env_is_true<K: AsRef<std::ffi::OsStr>>(key: K) -> bool {
	std::env::var(key).map_or(false, |v| {
		let v = v.trim().to_lowercase();
		!v.is_empty() && v != "0" && v != "f" && v != "false" && v != "n" && v != "no"
	})
}
