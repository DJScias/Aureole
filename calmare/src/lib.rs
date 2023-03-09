#![feature(let_chains)]
#![feature(pattern)]
#![feature(decl_macro)]
#![feature(try_blocks)]

pub mod ed6;
pub mod ed7;
mod writer;
pub mod common;
pub mod util;

pub use writer::Context;

pub mod span;
pub mod parse;

#[derive(Debug, Clone)]
pub enum Content {
	ED6Scena(themelios::scena::ed6::Scena),
	ED7Scena(themelios::scena::ed7::Scena),
}
