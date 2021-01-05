pub struct PeekIterator<T, It: Iterator<Item = T>> {
    iter: It,
    next_item: Option<Option<T>>,
}

impl <T, It: Iterator<Item = T>> PeekIterator<T, It> {
    pub fn new(iterator: It) -> Self {
        Self {
            iter: iterator,
            next_item: None,
        }
    }

    pub fn peek<'a>(&'a mut self) -> &'a Option<T> {
        if let None = self.next_item {
            self.next_item = Some(self.iter.next());
        }

        if let Some(res) = &self.next_item {
            res
        }
        else {
            panic!("Should never happen!");
        }
    }
}

impl <T, It: Iterator<Item = T>> From<It> for PeekIterator<T, It> {
    fn from(it: It) -> Self {
        Self::new(it)
    }
}

impl <T, It: Iterator<Item = T>> Iterator for PeekIterator<T, It> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(_) = self.next_item {
            let mut result = None;
            core::mem::swap(&mut self.next_item, &mut result);
            result.unwrap()
        }
        else {
            self.iter.next()
        }
    }
}
