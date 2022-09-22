use hamu::read::coverage::Coverage;
use hamu::read::le::*;
use hamu::write::le::*;
use crate::archive::Archives;
use crate::util::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct World { scena: String, x: u32, y: u32 }

pub fn read(_arcs: &Archives, data: &[u8]) -> Result<Vec<World>, ReadError> {
	let mut f = Coverage::new(Bytes::new(data));
	let mut table = Vec::with_capacity(f.remaining() / 4);
	while f.remaining() > 12 {
		let scena = _arcs.name(f.array()?)?.to_owned();
		let x = f.u32()?;
		let y = f.u32()?;
		table.push(World { scena, x, y });
	}
	f.check_u32(0xFFFFFFFF)?;
	f.check_u32(0)?;
	f.check_u32(0)?;
	f.assert_covered()?;
	Ok(table)
}

pub fn write(_arcs: &Archives, table: &Vec<World>) -> Result<Vec<u8>, WriteError> {
	let mut out = Out::<()>::new();
	for &World { ref scena, x, y } in table {
		out.array(_arcs.index(scena).unwrap());
		out.u32(x);
		out.u32(y);
	}
	out.u32(0xFFFFFFFF);
	out.u32(0);
	out.u32(0);
	Ok(out.finish()?)
}

#[cfg(test)]
mod test {
	use crate::archive::Archives;
	use crate::util::test::*;

	#[test_case::test_case(&FC; "fc")]
	fn roundtrip(arc: &Archives) -> Result<(), Error> {
		check_roundtrip(arc, "t_world._dt", super::read, super::write)?;
		Ok(())
	}
}
