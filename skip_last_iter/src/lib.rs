// mod skip_last_trait;
// pub use skip_last_trait::SkipLastTrait;
use std::collections::VecDeque;

pub struct SkipLastIterator<It: Iterator> {
    iter: It,
    count: usize,
    buffer: VecDeque<It::Item>,
}

impl <It: Iterator> SkipLastIterator<It> {
    pub fn new(iter: It, count: usize) -> Self {
        Self {
            iter,
            count,
            buffer: VecDeque::new(),
        }
    }
}

impl <It: Iterator> Iterator for SkipLastIterator<It> {
    type Item = It::Item;

    fn next(&mut self) -> Option<Self::Item> {
        while self.buffer.len() <= self.count {
            if let Some(elem) = self.iter.next() {
                self.buffer.push_back(elem)
            }
            else {
                return None
            }
        }
        self.buffer.pop_front()
    }
}
