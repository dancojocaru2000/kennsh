pub(crate) fn color_test() -> crate::Result<u8> {
	println!("Printing a full color test");
	println!();
	println!("Normal text");
	println!("\x1b[7mInverted text\x1b[m");
	println!();
	println!("4-bit colors");
	print!("Normal FG  ");
	for i in 30..=37 {
		print!("\x1b[{}m\x1b[107m {:>3} \x1b[0m", i, i);
	}
	println!();
	print!("Normal BG  ");
	for i in 30..=37 {
		print!("\x1b[{}m\x1b[97m {:>3} \x1b[0m", i + 10, i + 10);
	}
	println!();
	print!("Bright FG  ");
	for i in 90..=97 {
		print!("\x1b[{}m\x1b[40m {:>3} \x1b[0m", i, i);
	}
	println!();
	print!("Bright BG  ");
	for i in 90..=97 {
		print!("\x1b[{}m\x1b[30m {:>3} \x1b[0m", i + 10, i + 10);
	}
	println!();

	println!();
	
	println!("8-bit colors");
	for magnitude in 0..6 {
		let base = 16 + magnitude * 6;
		for i in 0..6 {
			let first = base + 36 * i;
			for j in 0..6 {
				let num = first + j;
				print!("\x1b[38;5;{}m\x1b[{}m {:>3} \x1b[0m", num, if magnitude < 3 { 107 } else { 40 }, num);
			}
			print!("   ");
			for j in 0..6 {
				let num = first + j;
				print!("\x1b[48;5;{}m\x1b[{}m {:>3} \x1b[0m", num, if magnitude < 3 { 97 } else { 30 }, num);
			}
			println!();
		}
	}
	for i in 0..4 {
		let first = 232 + 6 * i;
		for j in 0..6 {
			let num = first + j;
			print!("\x1b[38;5;{}m\x1b[{}m {:>3} \x1b[0m", num, if i < 2 { 107 } else { 40 }, num);
		}
		print!("   ");
		for j in 0..6 {
			let num = first + j;
			print!("\x1b[48;5;{}m\x1b[{}m {:>3} \x1b[0m", num, if i < 2 { 97 } else { 30 }, num);
		}
		println!();
	}
	println!();

	Ok(0)
}
