pub trait StringExtensions {
	fn is_blank(&self) -> bool;
	fn split_at_multiple<'a, Positions: AsRef<[usize]>>(&'a self, positions: &'a Positions) -> SplitAt<'a>;
	// fn as_utf8<'a>(&'a self) -> &'a dyn Iterator<Item = char>;
}

impl StringExtensions for String {
    fn is_blank(&self) -> bool {
        self.as_str().is_blank()
	}

	fn split_at_multiple<'a, Positions: AsRef<[usize]>>(&'a self, positions: &'a Positions) -> SplitAt<'a> {
		SplitAt {
			string: self,
			positions: positions.as_ref(),
			current_index: 0,
		}
	}

	// fn as_utf8<'a>(&'a self) -> &'a dyn Iterator<Item = char> {
	// 	&self.as_bytes().into_iter().map(|b| char::from(*b))
	// }
}

impl StringExtensions for &str {
    fn is_blank(&self) -> bool {
        for c in self.chars() {
			if !c.is_whitespace() {
				return false;
			}
		}
		true
	}

	fn split_at_multiple<'a, Positions: AsRef<[usize]>>(&'a self, positions: &'a Positions) -> SplitAt<'a> {
		SplitAt {
			string: self,
			positions: positions.as_ref(),
			current_index: 0,
		}
	}

	// fn as_utf8<'a>(&'a self) -> &'a dyn Iterator<Item = char> {
	// 	let bytes = self.as_bytes();
	// 	let it = bytes.into_iter();
	// 	let map = it.map(|b| char::from(*b));
	// 	map.into_iter()
	// }
}

pub struct SplitAt<'a> {
	string: &'a str,
	positions: &'a [usize],
	current_index: usize,
}

impl <'a> Iterator for SplitAt<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
		if self.current_index > self.positions.len() {
			None
		}
		else {
			Some({
				let previous_position = if self.current_index == 0 { 0 } else { self.positions[self.current_index - 1] + 1 };
				let current_position = if self.current_index == self.positions.len() { self.string.len() } else { self.positions[self.current_index] };
				let current_slice = &self.string[previous_position..current_position];
				self.current_index += 1;
				current_slice
			})
		}
    }
}
