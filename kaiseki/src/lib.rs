#![allow(dead_code)]
#![allow(clippy::needless_question_mark)]

pub mod ed6 {
	pub mod archive;
	pub mod magic;
	pub mod scena;
	pub use archive::{Archive,Archives};
}
mod decompress;
mod util;

pub use util::ByteString;
