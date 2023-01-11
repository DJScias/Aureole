// Made according to this guide https://zork.net/~st/jottings/sais.html
use std::cmp::Ordering;

trait Value: Ord + Clone + Into<usize> {
	fn i(&self) -> usize {
		self.clone().into()
	}
}
impl<T: Ord + Clone + Into<usize>> Value for T {}

// I will probably rename these to L and G later
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Type {
	S, // smaller
	L, // larger
}

fn types(data: &[impl Ord]) -> Vec<Type> { // TODO use bitvec?
	let mut res = vec![Type::S; data.len() + 1];
	if res.len() > 1 {
		res[data.len()-1] = Type::L;
		for i in (0..res.len()-2).rev() {
			res[i] = match data[i].cmp(&data[i+1]) {
				Ordering::Less => Type::S,
				Ordering::Equal => res[i+1],
				Ordering::Greater => Type::L,
			}
		}
	}
	res
}

fn is_lms(types: &[Type], pos: usize) -> bool {
	pos > 0 && types[pos] == Type::S && types[pos-1] == Type::L
}

fn bucket_sizes(data: &[impl Value], count: usize) -> Vec<usize> {
	let mut res = vec![0; count];
	for b in data {
		res[b.i()] += 1;
	}
	res
}

fn heads(buckets: &[usize]) -> Vec<usize> {
	let mut r = Vec::with_capacity(buckets.len());
	let mut o = 1;
	for &b in buckets {
		r.push(o);
		o += b;
	}
	r
}

fn tails(buckets: &[usize]) -> Vec<usize> {
	let mut r = Vec::with_capacity(buckets.len());
	let mut o = 1;
	for &b in buckets {
		o += b;
		r.push(o);
	}
	r
}

pub fn make_suffix_array(data: &[u8]) -> Vec<usize> {
	make_array(data, 256)
}

fn make_array(data: &[impl Value], count: usize) -> Vec<usize> {
	let buckets = bucket_sizes(data, count);
	let types = types(data);
	let mut guess = guess_lms_sort(data, &buckets, &types);
	induce_sort(data, &mut guess, &buckets, &types);
	let (summary, summary_size, summary_offsets) = summarize_array(data, &guess, &types);
	let summary_array = make_summary_array(&summary, summary_size);
	let mut result = accurate_lms_sort(data, &buckets, &summary_array, &summary_offsets);
	induce_sort(data, &mut result, &buckets, &types);
	result
}

fn guess_lms_sort(data: &[impl Value], buckets: &[usize], types: &[Type]) -> Vec<usize> {
	let mut result = vec![usize::MAX; data.len()+1];
	result[0] = data.len();
	let mut tails = tails(buckets);
	for (i, c) in data.iter().enumerate() {
		if is_lms(types, i) {
			tails[c.i()] -= 1;
			result[tails[c.i()]] = i;
		}
	}
	result
}

fn induce_sort(data: &[impl Value], guess: &mut [usize], buckets: &[usize], types: &[Type]) {
	let mut heads = heads(buckets);
	for i in 0..guess.len() {
		if guess[i] != usize::MAX && guess[i] != 0 && types[guess[i]-1] == Type::L {
			let v = &mut heads[data[guess[i]-1].i()];
			debug_assert!(*v > i);
			guess[*v] = guess[i] - 1;
			*v += 1;
		}
	}

	let mut tails = tails(buckets);
	for i in (0..guess.len()).rev() {
		if guess[i] != usize::MAX && guess[i] != 0 && types[guess[i]-1] == Type::S {
			let v = &mut tails[data[guess[i]-1].i()];
			debug_assert!(*v <= i);
			*v -= 1;
			guess[*v] = guess[i] - 1;
		}
	}
}

fn summarize_array(data: &[impl Value], guess: &[usize], types: &[Type]) -> (Vec<usize>, usize, Vec<usize>) {
	let mut names = vec![usize::MAX; data.len()+1];
	let mut cur_name = 0;
	names[guess[0]] = cur_name;
	let mut last_offset = guess[0];
	for &offset in &guess[1..] {
		if is_lms(types, offset) {
			if !lms_equal(data, types, last_offset, offset) {
				cur_name += 1
			}
			last_offset = offset;
			names[offset] = cur_name;
		}
	}

	let (summary_offsets, summary) = names.into_iter()
		.enumerate()
		.filter(|a| a.1 != usize::MAX)
		.unzip();
	(summary, cur_name + 1, summary_offsets)
}

fn lms_equal(data: &[impl Value], types: &[Type], a: usize, b: usize) -> bool {
	if a == data.len() || b == data.len() {
		return a == b
	}
	let mut i = 0;
	loop {
		let al = is_lms(types, a+i);
		let bl = is_lms(types, b+i);
		if i > 0 && al && bl {
			return true
		}
		if al != bl {
			return false
		}
		if data[a+i] != data[b+i] {
			return false
		}
		i += 1
	}
}


fn make_summary_array(summary: &[usize], summary_size: usize) -> Vec<usize> {
	if summary.len() == summary_size {
		let mut array = vec![usize::MAX; summary.len()+1];
		array[0] = summary.len();
		for (i, &c) in summary.iter().enumerate() {
			array[c+1] = i
		}
		array
	} else {
		make_array(summary, summary_size)
	}
}

fn accurate_lms_sort(
	data: &[impl Value],
	buckets: &[usize],
	summary_array: &[usize],
	summary_offsets: &[usize],
) -> Vec<usize> {
	let mut array = vec![usize::MAX; data.len()+1];
	array[0] = data.len();
	let mut tails = tails(buckets);
	for i in (2..summary_array.len()).rev() {
		let si = summary_offsets[summary_array[i]];
		let v = &mut tails[data[si].i()];
		*v -= 1;
		array[*v] = si;
	}
	array
}

#[test]
fn a() {
	fn show(a: &[usize]) {
		let a: &[isize] = unsafe { std::mem::transmute(a) };
		println!("{:02?}", a);
	}
	show(&make_array("cabbage".as_bytes(), 256));
	show(&make_array("baabaabac".as_bytes(), 256));
}

