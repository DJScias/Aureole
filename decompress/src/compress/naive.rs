use super::common::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompressCommand {
	Repeat(usize, usize),
	Byte(u8),
}

struct RepeatFinder<'a> {
	data: &'a [u8],
	pos: usize,
	table: Vec<usize>, // a suffix array, constructed incrementally

	max_repeat: usize,
}

impl<'a> RepeatFinder<'a> {
	fn new(data: &'a [u8], max_repeat: usize) -> Self {
		Self { data, pos: 0, table: Vec::new(), max_repeat }
	}
}

impl Iterator for RepeatFinder<'_> {
	type Item = CompressCommand;

	fn next(&mut self) -> Option<CompressCommand> {
		if self.pos == self.data.len() {
			return None
		}

		let s = |x| &self.data[x..];

		let p = self.table.binary_search_by_key(&s(self.pos), |a| s(*a)).unwrap_err();

		let mut m = (0, 0);

		if let Some(&x) = p.checked_sub(1).and_then(|p| self.table.get(p)) {
			let n = s(x).iter().zip(s(self.pos).iter()).take_while(|(a,b)| a == b).count();
			m = m.max((n, x));
		}

		if let Some(&x) = self.table.get(p) {
			let n = s(x).iter().zip(s(self.pos).iter()).take_while(|(a,b)| a == b).count();
			m = m.max((n, x));
		}

		let (cmd, n) = if m.0 > 1 {
			(CompressCommand::Repeat(self.pos - m.1, m.0), m.0)
		} else {
			(CompressCommand::Byte(self.data[self.pos]), 1)
		};

		for _ in 0..n {
			let p = self.table.binary_search_by_key(&s(self.pos), |a| s(*a)).unwrap_err();
			self.table.insert(p, self.pos);
			self.pos += 1;
			if self.pos >= self.max_repeat {
				let v = self.pos - self.max_repeat;
				let p = self.table.binary_search_by(|a| {
					if *a == v {
						std::cmp::Ordering::Equal
					} else {
						s(*a).cmp(s(v))
					}
				}).unwrap();
				assert_eq!(self.table.remove(p), v);
			}
		}

		Some(cmd)
	}
}

pub fn compress_chunk(data: &[u8]) -> Vec<u8> {
	let mut b = BitW::new();
	for item in RepeatFinder::new(data, 1<<13) {
		match item {
			CompressCommand::Byte(x) => {
				b.bit(false);
				b.bits(8, x as usize);
			}
			CompressCommand::Repeat(o, mut n) => {
				loop {
					b.bit(true);
					b.bit(o >= 256);
					if o < 256 {
						b.bits(8, o);
					} else {
						b.bits(13, o)
					}

					for i in 2..=5 {
						if n > i {
							b.bit(false);
						}
					}
					if n < 6 {
						b.bit(true);
					} else if n < 14 {
						b.bit(true);
						b.bits(3, n-6);
					} else {
						b.bit(false);
						if n == 270 { // if we do 269 here we'll get a 1-chunk, don't want that
							b.bits(8, 268-14);
							n -= 268;
							continue
						} else if n >= 270 {
							b.bits(8, 269-14);
							n -= 269;
							continue
						} else {
							b.bits(8, n-14);
						}
					}
					break
				}
			}
		}
	}
	b.bit(true);
	b.bit(true);
	b.bits(13, 0);
	b.finish()
}
