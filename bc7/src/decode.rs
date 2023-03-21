// BC7 graphics format implementation
//
// Based on the one by Lucas Himbert,
//   https://git.sr.ht/~quf/tocs/tree/1889f61412909a26ef164b5a7065dcd799bfcacc/item/tocs/src/bc7.rs
//
// Specification: https://registry.khronos.org/DataFormat/specs/1.3/dataformat.1.3.html#bptc_bc7

pub type Rgba = [u8; 4];

pub fn decode(block: u128) -> [[Rgba; 4]; 4] {
	match block.trailing_zeros() as usize {
		// Table 109. Mode-dependent BPTC parameters
		//                     NS  PB  RS ISB  CB  AB EPB SPB  IB  IB₂
		0 => decode_with_mode::<3,  4,  0,  0,  4,  0,  1,  0,  3,  0>(block>>1),
		1 => decode_with_mode::<2,  6,  0,  0,  6,  0,  0,  1,  3,  0>(block>>2),
		2 => decode_with_mode::<3,  6,  0,  0,  5,  0,  0,  0,  2,  0>(block>>3),
		3 => decode_with_mode::<2,  6,  0,  0,  7,  0,  1,  0,  2,  0>(block>>4),
		4 => decode_with_mode::<1,  0,  2,  1,  5,  6,  0,  0,  2,  3>(block>>5),
		5 => decode_with_mode::<1,  0,  2,  0,  7,  8,  0,  0,  2,  2>(block>>6),
		6 => decode_with_mode::<1,  0,  0,  0,  7,  7,  1,  0,  4,  0>(block>>7),
		7 => decode_with_mode::<2,  6,  0,  0,  5,  5,  1,  0,  2,  0>(block>>8),
		_ => [[Rgba::default(); 4]; 4],
	}
}

#[inline(always)]
fn get(bits: &mut u128, nbits: usize) -> u8 {
	let x = (1u128 << nbits) - 1;
	let v = (*bits & x) as u8;
	*bits >>= nbits;
	v
}

#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn decode_with_mode<
	// Table 110. Full descriptions of the BPTC mode columns
	const NS:  usize, // Number of subsets
	const PB:  usize, // Partition selection bits
	const RB:  usize, // Rotation bits
	const ISB: usize, // Index selection bit
	const CB:  usize, // Color bits
	const AB:  usize, // Alpha bits
	const EPB: usize, // Endpoint P-bits (all channels)
	const SPB: usize, // Shared P-bits
	const IB:  usize, // Index bits
	const IB2: usize, // Secondary index bits
>(bits: u128) -> [[Rgba; 4]; 4] {
	let bits = &mut {bits};
	let partition = get(bits, PB) as usize;
	let rotation  = get(bits, RB);
	let index_sel = get(bits, ISB) != 0;

	let endpoints = &mut [[Rgba::default(); 2]; NS];

	let cab = [CB, CB, CB, AB];

	#[allow(clippy::needless_range_loop)]
	for c in 0..4 {
		for endp in endpoints.iter_mut() {
			endp[0][c] = get(bits, cab[c]);
			endp[1][c] = get(bits, cab[c]);
		}
	}

	for endp in endpoints.iter_mut() {
		for e in endp.iter_mut() {
			let p = get(bits, EPB);
			*e = e.map(|c| (c << EPB) | p);
		}
	}

	for endp in endpoints.iter_mut() {
		let p = get(bits, SPB);
		for e in endp.iter_mut() {
			*e = e.map(|c| (c << SPB) | p);
		}
	}

	for c in 0..4 {
		for e in endpoints.iter_mut().flatten() {
			let total_bits = cab[c] + EPB + SPB;
			if 0 < total_bits && total_bits < 8 {
				e[c] <<= 8 - total_bits;
				e[c] |= e[c] >> total_bits;
			}
		}
	}

	let cbits = &mut {*bits};
	let abits = &mut {*bits >> (16 * IB - NS)};

	let mut output = [[Rgba::default(); 4]; 4];

	for (y, row) in output.iter_mut().enumerate() {
		for (x, px) in row.iter_mut().enumerate() {
			let (subset_index, is_anchor) = subset_index::<NS>(partition, x, y);
			let endp = endpoints[subset_index];

			let i0 = get(cbits, IB - usize::from(is_anchor)) as usize;
			if IB2 > 0 {
				let i1 = get(abits, IB2 - usize::from((x, y) == (0, 0))) as usize;
				if index_sel {
					px[0] = interpolate::<IB2>(endp[0][0], endp[1][0], i1);
					px[1] = interpolate::<IB2>(endp[0][1], endp[1][1], i1);
					px[2] = interpolate::<IB2>(endp[0][2], endp[1][2], i1);
					px[3] = interpolate::<IB >(endp[0][3], endp[1][3], i0);
				} else {
					px[0] = interpolate::<IB >(endp[0][0], endp[1][0], i0);
					px[1] = interpolate::<IB >(endp[0][1], endp[1][1], i0);
					px[2] = interpolate::<IB >(endp[0][2], endp[1][2], i0);
					px[3] = interpolate::<IB2>(endp[0][3], endp[1][3], i1);
				}
			} else {
				px[0] = interpolate::<IB>(endp[0][0], endp[1][0], i0);
				px[1] = interpolate::<IB>(endp[0][1], endp[1][1], i0);
				px[2] = interpolate::<IB>(endp[0][2], endp[1][2], i0);
				px[3] = interpolate::<IB>(endp[0][3], endp[1][3], i0);
			};

			if AB == 0 {
				px[3] = 255;
			}

			match rotation {
				// Table 120. BPTC Rotation bits
				0 => (), // No change
				1 => px.swap(3, 0),
				2 => px.swap(3, 1),
				3 => px.swap(3, 2),
				_ => unreachable!(),
			};
		}
	}

	output
}

fn subset_index<const NS: usize>(partition: usize, x: usize, y: usize) -> (usize, bool) {
	// Table 114. Partition table for 2-subset BPTC, with the 4×4 block of values for each partition number
	const P2: [[[u8; 4]; 4]; 64] = [
		[[0, 0, 1, 1], [0, 0, 1, 1], [0, 0, 1, 1], [0, 0, 1, 1]],
		[[0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 1]],
		[[0, 1, 1, 1], [0, 1, 1, 1], [0, 1, 1, 1], [0, 1, 1, 1]],
		[[0, 0, 0, 1], [0, 0, 1, 1], [0, 0, 1, 1], [0, 1, 1, 1]],
		[[0, 0, 0, 0], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 1, 1]],
		[[0, 0, 1, 1], [0, 1, 1, 1], [0, 1, 1, 1], [1, 1, 1, 1]],
		[[0, 0, 0, 1], [0, 0, 1, 1], [0, 1, 1, 1], [1, 1, 1, 1]],
		[[0, 0, 0, 0], [0, 0, 0, 1], [0, 0, 1, 1], [0, 1, 1, 1]],
		[[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 1], [0, 0, 1, 1]],
		[[0, 0, 1, 1], [0, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 1]],
		[[0, 0, 0, 0], [0, 0, 0, 1], [0, 1, 1, 1], [1, 1, 1, 1]],
		[[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 1], [0, 1, 1, 1]],
		[[0, 0, 0, 1], [0, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 1]],
		[[0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1], [1, 1, 1, 1]],
		[[0, 0, 0, 0], [1, 1, 1, 1], [1, 1, 1, 1], [1, 1, 1, 1]],
		[[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1]],
		[[0, 0, 0, 0], [1, 0, 0, 0], [1, 1, 1, 0], [1, 1, 1, 1]],
		[[0, 1, 1, 1], [0, 0, 0, 1], [0, 0, 0, 0], [0, 0, 0, 0]],
		[[0, 0, 0, 0], [0, 0, 0, 0], [1, 0, 0, 0], [1, 1, 1, 0]],
		[[0, 1, 1, 1], [0, 0, 1, 1], [0, 0, 0, 1], [0, 0, 0, 0]],
		[[0, 0, 1, 1], [0, 0, 0, 1], [0, 0, 0, 0], [0, 0, 0, 0]],
		[[0, 0, 0, 0], [1, 0, 0, 0], [1, 1, 0, 0], [1, 1, 1, 0]],
		[[0, 0, 0, 0], [0, 0, 0, 0], [1, 0, 0, 0], [1, 1, 0, 0]],
		[[0, 1, 1, 1], [0, 0, 1, 1], [0, 0, 1, 1], [0, 0, 0, 1]],
		[[0, 0, 1, 1], [0, 0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 0]],
		[[0, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0], [1, 1, 0, 0]],
		[[0, 1, 1, 0], [0, 1, 1, 0], [0, 1, 1, 0], [0, 1, 1, 0]],
		[[0, 0, 1, 1], [0, 1, 1, 0], [0, 1, 1, 0], [1, 1, 0, 0]],
		[[0, 0, 0, 1], [0, 1, 1, 1], [1, 1, 1, 0], [1, 0, 0, 0]],
		[[0, 0, 0, 0], [1, 1, 1, 1], [1, 1, 1, 1], [0, 0, 0, 0]],
		[[0, 1, 1, 1], [0, 0, 0, 1], [1, 0, 0, 0], [1, 1, 1, 0]],
		[[0, 0, 1, 1], [1, 0, 0, 1], [1, 0, 0, 1], [1, 1, 0, 0]],
		[[0, 1, 0, 1], [0, 1, 0, 1], [0, 1, 0, 1], [0, 1, 0, 1]],
		[[0, 0, 0, 0], [1, 1, 1, 1], [0, 0, 0, 0], [1, 1, 1, 1]],
		[[0, 1, 0, 1], [1, 0, 1, 0], [0, 1, 0, 1], [1, 0, 1, 0]],
		[[0, 0, 1, 1], [0, 0, 1, 1], [1, 1, 0, 0], [1, 1, 0, 0]],
		[[0, 0, 1, 1], [1, 1, 0, 0], [0, 0, 1, 1], [1, 1, 0, 0]],
		[[0, 1, 0, 1], [0, 1, 0, 1], [1, 0, 1, 0], [1, 0, 1, 0]],
		[[0, 1, 1, 0], [1, 0, 0, 1], [0, 1, 1, 0], [1, 0, 0, 1]],
		[[0, 1, 0, 1], [1, 0, 1, 0], [1, 0, 1, 0], [0, 1, 0, 1]],
		[[0, 1, 1, 1], [0, 0, 1, 1], [1, 1, 0, 0], [1, 1, 1, 0]],
		[[0, 0, 0, 1], [0, 0, 1, 1], [1, 1, 0, 0], [1, 0, 0, 0]],
		[[0, 0, 1, 1], [0, 0, 1, 0], [0, 1, 0, 0], [1, 1, 0, 0]],
		[[0, 0, 1, 1], [1, 0, 1, 1], [1, 1, 0, 1], [1, 1, 0, 0]],
		[[0, 1, 1, 0], [1, 0, 0, 1], [1, 0, 0, 1], [0, 1, 1, 0]],
		[[0, 0, 1, 1], [1, 1, 0, 0], [1, 1, 0, 0], [0, 0, 1, 1]],
		[[0, 1, 1, 0], [0, 1, 1, 0], [1, 0, 0, 1], [1, 0, 0, 1]],
		[[0, 0, 0, 0], [0, 1, 1, 0], [0, 1, 1, 0], [0, 0, 0, 0]],
		[[0, 1, 0, 0], [1, 1, 1, 0], [0, 1, 0, 0], [0, 0, 0, 0]],
		[[0, 0, 1, 0], [0, 1, 1, 1], [0, 0, 1, 0], [0, 0, 0, 0]],
		[[0, 0, 0, 0], [0, 0, 1, 0], [0, 1, 1, 1], [0, 0, 1, 0]],
		[[0, 0, 0, 0], [0, 1, 0, 0], [1, 1, 1, 0], [0, 1, 0, 0]],
		[[0, 1, 1, 0], [1, 1, 0, 0], [1, 0, 0, 1], [0, 0, 1, 1]],
		[[0, 0, 1, 1], [0, 1, 1, 0], [1, 1, 0, 0], [1, 0, 0, 1]],
		[[0, 1, 1, 0], [0, 0, 1, 1], [1, 0, 0, 1], [1, 1, 0, 0]],
		[[0, 0, 1, 1], [1, 0, 0, 1], [1, 1, 0, 0], [0, 1, 1, 0]],
		[[0, 1, 1, 0], [1, 1, 0, 0], [1, 1, 0, 0], [1, 0, 0, 1]],
		[[0, 1, 1, 0], [0, 0, 1, 1], [0, 0, 1, 1], [1, 0, 0, 1]],
		[[0, 1, 1, 1], [1, 1, 1, 0], [1, 0, 0, 0], [0, 0, 0, 1]],
		[[0, 0, 0, 1], [1, 0, 0, 0], [1, 1, 1, 0], [0, 1, 1, 1]],
		[[0, 0, 0, 0], [1, 1, 1, 1], [0, 0, 1, 1], [0, 0, 1, 1]],
		[[0, 0, 1, 1], [0, 0, 1, 1], [1, 1, 1, 1], [0, 0, 0, 0]],
		[[0, 0, 1, 0], [0, 0, 1, 0], [1, 1, 1, 0], [1, 1, 1, 0]],
		[[0, 1, 0, 0], [0, 1, 0, 0], [0, 1, 1, 1], [0, 1, 1, 1]],
	];
	// Table 115. Partition table for 3-subset BPTC, with the 4×4 block of values for each partition number
	const P3: [[[u8; 4]; 4]; 64] = [
		[[0, 0, 1, 1], [0, 0, 1, 1], [0, 2, 2, 1], [2, 2, 2, 2]],
		[[0, 0, 0, 1], [0, 0, 1, 1], [2, 2, 1, 1], [2, 2, 2, 1]],
		[[0, 0, 0, 0], [2, 0, 0, 1], [2, 2, 1, 1], [2, 2, 1, 1]],
		[[0, 2, 2, 2], [0, 0, 2, 2], [0, 0, 1, 1], [0, 1, 1, 1]],
		[[0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 2, 2], [1, 1, 2, 2]],
		[[0, 0, 1, 1], [0, 0, 1, 1], [0, 0, 2, 2], [0, 0, 2, 2]],
		[[0, 0, 2, 2], [0, 0, 2, 2], [1, 1, 1, 1], [1, 1, 1, 1]],
		[[0, 0, 1, 1], [0, 0, 1, 1], [2, 2, 1, 1], [2, 2, 1, 1]],
		[[0, 0, 0, 0], [0, 0, 0, 0], [1, 1, 1, 1], [2, 2, 2, 2]],
		[[0, 0, 0, 0], [1, 1, 1, 1], [1, 1, 1, 1], [2, 2, 2, 2]],
		[[0, 0, 0, 0], [1, 1, 1, 1], [2, 2, 2, 2], [2, 2, 2, 2]],
		[[0, 0, 1, 2], [0, 0, 1, 2], [0, 0, 1, 2], [0, 0, 1, 2]],
		[[0, 1, 1, 2], [0, 1, 1, 2], [0, 1, 1, 2], [0, 1, 1, 2]],
		[[0, 1, 2, 2], [0, 1, 2, 2], [0, 1, 2, 2], [0, 1, 2, 2]],
		[[0, 0, 1, 1], [0, 1, 1, 2], [1, 1, 2, 2], [1, 2, 2, 2]],
		[[0, 0, 1, 1], [2, 0, 0, 1], [2, 2, 0, 0], [2, 2, 2, 0]],
		[[0, 0, 0, 1], [0, 0, 1, 1], [0, 1, 1, 2], [1, 1, 2, 2]],
		[[0, 1, 1, 1], [0, 0, 1, 1], [2, 0, 0, 1], [2, 2, 0, 0]],
		[[0, 0, 0, 0], [1, 1, 2, 2], [1, 1, 2, 2], [1, 1, 2, 2]],
		[[0, 0, 2, 2], [0, 0, 2, 2], [0, 0, 2, 2], [1, 1, 1, 1]],
		[[0, 1, 1, 1], [0, 1, 1, 1], [0, 2, 2, 2], [0, 2, 2, 2]],
		[[0, 0, 0, 1], [0, 0, 0, 1], [2, 2, 2, 1], [2, 2, 2, 1]],
		[[0, 0, 0, 0], [0, 0, 1, 1], [0, 1, 2, 2], [0, 1, 2, 2]],
		[[0, 0, 0, 0], [1, 1, 0, 0], [2, 2, 1, 0], [2, 2, 1, 0]],
		[[0, 1, 2, 2], [0, 1, 2, 2], [0, 0, 1, 1], [0, 0, 0, 0]],
		[[0, 0, 1, 2], [0, 0, 1, 2], [1, 1, 2, 2], [2, 2, 2, 2]],
		[[0, 1, 1, 0], [1, 2, 2, 1], [1, 2, 2, 1], [0, 1, 1, 0]],
		[[0, 0, 0, 0], [0, 1, 1, 0], [1, 2, 2, 1], [1, 2, 2, 1]],
		[[0, 0, 2, 2], [1, 1, 0, 2], [1, 1, 0, 2], [0, 0, 2, 2]],
		[[0, 1, 1, 0], [0, 1, 1, 0], [2, 0, 0, 2], [2, 2, 2, 2]],
		[[0, 0, 1, 1], [0, 1, 2, 2], [0, 1, 2, 2], [0, 0, 1, 1]],
		[[0, 0, 0, 0], [2, 0, 0, 0], [2, 2, 1, 1], [2, 2, 2, 1]],
		[[0, 0, 0, 0], [0, 0, 0, 2], [1, 1, 2, 2], [1, 2, 2, 2]],
		[[0, 2, 2, 2], [0, 0, 2, 2], [0, 0, 1, 2], [0, 0, 1, 1]],
		[[0, 0, 1, 1], [0, 0, 1, 2], [0, 0, 2, 2], [0, 2, 2, 2]],
		[[0, 1, 2, 0], [0, 1, 2, 0], [0, 1, 2, 0], [0, 1, 2, 0]],
		[[0, 0, 0, 0], [1, 1, 1, 1], [2, 2, 2, 2], [0, 0, 0, 0]],
		[[0, 1, 2, 0], [1, 2, 0, 1], [2, 0, 1, 2], [0, 1, 2, 0]],
		[[0, 1, 2, 0], [2, 0, 1, 2], [1, 2, 0, 1], [0, 1, 2, 0]],
		[[0, 0, 1, 1], [2, 2, 0, 0], [1, 1, 2, 2], [0, 0, 1, 1]],
		[[0, 0, 1, 1], [1, 1, 2, 2], [2, 2, 0, 0], [0, 0, 1, 1]],
		[[0, 1, 0, 1], [0, 1, 0, 1], [2, 2, 2, 2], [2, 2, 2, 2]],
		[[0, 0, 0, 0], [0, 0, 0, 0], [2, 1, 2, 1], [2, 1, 2, 1]],
		[[0, 0, 2, 2], [1, 1, 2, 2], [0, 0, 2, 2], [1, 1, 2, 2]],
		[[0, 0, 2, 2], [0, 0, 1, 1], [0, 0, 2, 2], [0, 0, 1, 1]],
		[[0, 2, 2, 0], [1, 2, 2, 1], [0, 2, 2, 0], [1, 2, 2, 1]],
		[[0, 1, 0, 1], [2, 2, 2, 2], [2, 2, 2, 2], [0, 1, 0, 1]],
		[[0, 0, 0, 0], [2, 1, 2, 1], [2, 1, 2, 1], [2, 1, 2, 1]],
		[[0, 1, 0, 1], [0, 1, 0, 1], [0, 1, 0, 1], [2, 2, 2, 2]],
		[[0, 2, 2, 2], [0, 1, 1, 1], [0, 2, 2, 2], [0, 1, 1, 1]],
		[[0, 0, 0, 2], [1, 1, 1, 2], [0, 0, 0, 2], [1, 1, 1, 2]],
		[[0, 0, 0, 0], [2, 1, 1, 2], [2, 1, 1, 2], [2, 1, 1, 2]],
		[[0, 2, 2, 2], [0, 1, 1, 1], [0, 1, 1, 1], [0, 2, 2, 2]],
		[[0, 0, 0, 2], [1, 1, 1, 2], [1, 1, 1, 2], [0, 0, 0, 2]],
		[[0, 1, 1, 0], [0, 1, 1, 0], [0, 1, 1, 0], [2, 2, 2, 2]],
		[[0, 0, 0, 0], [0, 0, 0, 0], [2, 1, 1, 2], [2, 1, 1, 2]],
		[[0, 1, 1, 0], [0, 1, 1, 0], [2, 2, 2, 2], [2, 2, 2, 2]],
		[[0, 0, 2, 2], [0, 0, 1, 1], [0, 0, 1, 1], [0, 0, 2, 2]],
		[[0, 0, 2, 2], [1, 1, 2, 2], [1, 1, 2, 2], [0, 0, 2, 2]],
		[[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [2, 1, 1, 2]],
		[[0, 0, 0, 2], [0, 0, 0, 1], [0, 0, 0, 2], [0, 0, 0, 1]],
		[[0, 2, 2, 2], [1, 2, 2, 2], [0, 2, 2, 2], [1, 2, 2, 2]],
		[[0, 1, 0, 1], [2, 2, 2, 2], [2, 2, 2, 2], [2, 2, 2, 2]],
		[[0, 1, 1, 1], [2, 0, 1, 1], [2, 2, 0, 1], [2, 2, 2, 0]],
	];

	// Table 116. BPTC anchor index values for the second subset of three-subset partitioning, by partition number
	const A3A: [u8; 64] = [
		3,  3,  15, 15, 8,  3,  15, 15,
		8,  8,  6,  6,  6,  5,  3,  3,
		3,  3,  8,  15, 3,  3,  6,  10,
		5,  8,  8,  6,  8,  5,  15, 15,
		8,  15, 3,  5,  6,  10, 8,  15,
		15, 3,  15, 5,  15, 15, 15, 15,
		3,  15, 5,  5,  5,  8,  5,  10,
		5,  10, 8,  13, 15, 12, 3,  3,
	];

	// Table 117. BPTC anchor index values for the third subset of three-subset partitioning, by partition number
	const A3B: [u8; 64] = [
		15, 8,  8,  3,  15, 15, 3,  8,
		15, 15, 15, 15, 15, 15, 15, 8,
		15, 8,  15, 3,  15, 8,  15, 8,
		3,  15, 6,  10, 15, 15, 10, 8,
		15, 3,  15, 10, 10, 8,  9,  10,
		6,  15, 8,  15, 3,  6,  6,  8,
		15, 3,  15, 15, 15, 15, 15, 15,
		15, 15, 15, 15, 3,  15, 15, 8,
	];

	// Table 118. BPTC anchor index values for the second subset of two-subset partitioning, by partition number
	const A2: [u8; 64] = [
		15, 15, 15, 15, 15, 15, 15, 15,
		15, 15, 15, 15, 15, 15, 15, 15,
		15, 2,  8,  2,  2,  8,  8,  15,
		2,  8,  2,  2,  8,  8,  2,  2,
		15, 15, 6,  8,  2,  8,  15, 15,
		2,  8,  2,  2,  2,  15, 15, 6,
		6,  2,  6,  8,  15, 15, 2,  2,
		15, 15, 15, 15, 15, 2,  2,  15,
	];

	let index = match NS {
		1 => 0,
		2 => P2[partition][y][x],
		3 => P3[partition][y][x],
		_ => unreachable!(),
	} as usize;

	let i = (y * 4 + x) as u8;
	let is_anchor = match NS {
		1 => i == 0,
		2 => i == 0 || i == A2[partition],
		3 => i == 0 || i == A3A[partition] || i == A3B[partition],
		_ => unreachable!(),
	};

	(index, is_anchor)
}

fn interpolate<const IB: usize>(e0: u8, e1: u8, i: usize) -> u8 {
	let weight = match IB {
		// Table 119. BPTC interpolation factors
		2 => [       0,             21,             43,             64      ][i],
		3 => [   0,      9,     18,     27,     37,     46,     55,     64  ][i],
		4 => [ 0,  4,  9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64][i],
		_ => unreachable!(),
	};
	let e0 = e0 as u16;
	let e1 = e1 as u16;
	// Equation 2. BPTC endpoint interpolation formula
	(((64 - weight) * e0 + weight * e1 + 32) >> 6) as u8
}
