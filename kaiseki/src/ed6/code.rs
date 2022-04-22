use std::collections::HashMap;
use eyre::Result;
use hamu::read::{In, Le};
use crate::util::{self, Text, InExt};

pub type Code = Vec<(usize, Insn)>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FileRef(pub u16, pub u16); // (index, arch)

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct FuncRef(pub u16, pub u16);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Pos3(pub i32, pub i32, pub i32);

impl std::fmt::Debug for FileRef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "FileRef({:#02X}, {})", self.0, self.1)
	}
}

impl std::fmt::Debug for FuncRef {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "FuncRef({}, {})", self.0, self.1)
	}
}

impl std::fmt::Debug for Pos3 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Pos3({}, {}, {})", self.0, self.1, self.2)
	}
}

#[extend::ext(name=InExtForCode)]
impl In<'_> {
	fn file_ref(&mut self) -> hamu::read::Result<FileRef> {
		Ok(FileRef(self.u16()?, self.u16()?))
	}

	fn func_ref(&mut self) -> hamu::read::Result<FuncRef> {
		Ok(FuncRef(self.u16()?, self.u16()?))
	}

	fn pos3(&mut self) -> hamu::read::Result<Pos3> {
		Ok(Pos3(self.i32()?, self.i32()?, self.i32()?))
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Character(pub u16);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExprBinop {
	Eq, Ne, Lt, Gt, Le, Ge,
	BoolAnd, And, Or,
	Add, Sub, Xor, Mul, Div, Mod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExprUnop {
	Not, Neg, Inv,
	Ass, MulAss, DivAss, ModAss, AddAss, SubAss, AndAss, XorAss, OrAss
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
	Const(u32),
	Binop(ExprBinop, Box<Expr>, Box<Expr>),
	Unop(ExprUnop, Box<Expr>),
	Exec(Insn),
	Flag(Flag),
	Var(u16 /*Var*/),
	Attr(u8 /*Attr*/),
	CharAttr(Character, u8 /*CharAttr*/),
	Rand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Insn {
	/*01*/ Return,
	/*02*/ If(Box<Expr>, usize /*Addr*/),
	/*03*/ Goto(usize /*Addr*/),
	/*04*/ Switch(Box<Expr>, Vec<(u16, usize /*Addr*/)>, usize /*Addr*/ /*(default)*/),
	/*08*/ Sleep(u32 /*Time*/),
	/*09*/ FlagsSet(u32 /*Flags*/),
	/*0A*/ FlagsUnset(u32 /*Flags*/),
	/*0B*/ FadeOn(u32 /*Time*/, u32 /*Color*/, u8),
	/*0C*/ FadeOff(u32 /*Time*/, u32 /*Color*/),
	/*0D*/ _0D,
	/*0F*/ Battle(u16 /*BattleId*/, u16, u16, u16, u8, u16, i8),
	/*16*/ Map(MapInsn),
	/*19*/ EventBegin(u8),
	/*1A*/ EventEnd(u8),
	/*1B*/ _1B(u16, u16),
	/*1C*/ _1C(u16, u16),
	/*22*/ SoundPlay(u16 /*Sound*/, u8, u8 /*Volume*/),
	/*23*/ SoundStop(u16 /*Sound*/),
	/*24*/ SoundLoop(u16 /*Sound*/, u8),
	/*28*/ Quest(u16 /*Quest*/, QuestInsn),
	/*29*/ QuestGet(u16 /*Quest*/, QuestGetInsn),
	/*30*/ _Party30(u8),
	/*43*/ CharForkFunc(Character, u8 /*ForkId*/, FuncRef),
	/*45*/ CharFork(Character, u16 /*ForkId*/, Vec<Insn>), // why is this is u16?
	/*49*/ Event(FuncRef), // Not sure if this is different from Call
	/*4D*/ ExprVar(u16 /*Var*/, Box<Expr>),
	/*4F*/ ExprAttr(u8 /*Attr*/, Box<Expr>),
	/*51*/ ExprCharAttr(Character, u8 /*CharAttr*/, Box<Expr>),
	/*53*/ TextEnd(Character),
	/*54*/ TextMessage(Text),
	/*56*/ TextReset(u8),
	/*58*/ TextWait,
	/*5A*/ TextSetPos(i16, i16, i16, i16),
	/*5B*/ TextTalk(Character, Text),
	/*5C*/ TextTalkNamed(Character, String, Text),
	/*5D*/ Menu(u16 /*MenuId*/, (i16, i16) /*Pos*/, u8, Vec<String>),
	/*5E*/ MenuWait(u16 /*MenuId*/),
	/*5F*/ _Menu5F(u16 /*MenuId*/), // MenuClose?
	/*60*/ TextSetName(String),
	/*69*/ CamLookAt(Character, u32 /*Time*/),
	/*6C*/ CamAngle(i32 /*Angle*/, u32 /*Time*/),
	/*6D*/ CamPos(Pos3, u32 /*Time*/),
	/*87*/ CharSetFrame(Character, u16),
	/*88*/ CharSetPos(Character, Pos3, u16 /*Angle*/),
	/*8A*/ CharLookAt(Character, Character, u16 /*Time*/),
	/*8E*/ CharWalkTo(Character, Pos3, u32 /*Speed*/, u8),
	/*90*/ CharWalk(Character, Pos3, u32 /*Speed*/, u8), // I don't know how this differs from CharWalkTo; is it relative maybe?
	/*92*/ _Char92(Character, Character, u32, u32, u8),
	/*99*/ CharAnimation(Character, u8, u8, u32 /*Time*/),
	/*9A*/ CharFlagsSet(Character, u16 /*CharFlags*/),
	/*9B*/ CharFlagsUnset(Character, u16 /*CharFlags*/),
	/*A2*/ FlagSet(Flag),
	/*A3*/ FlagUnset(Flag),
	/*A5*/ AwaitFlagUnset(Flag),
	/*A6*/ AwaitFlagSet(Flag),
	/*B1*/ OpLoad(String /*._OP filename*/),
	/*B2*/ _B2(u8, u8, u16),
	/*B4*/ ReturnToTitle(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapInsn {
	/*00*/ Hide,
	/*01*/ Show,
	/*02*/ Set(i32, (i32, i32), FileRef /* archive 03 */), // XXX this seems to be (arch, index) while others are (index, arch)?
}

// I am unsure whether these are Set or Unset
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestInsn {
	/*01*/ TaskSet(u16),
	/*02*/ TaskUnset(u16),
	/*03*/ FlagsSet(u8),
	/*04*/ FlagsUnset(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestGetInsn {
	/*00*/ Task(u16),
	/*01*/ Flags(u8),
}

pub struct CodeParser {
	marks: HashMap<usize, String>,
}

impl CodeParser {
	#[allow(clippy::new_without_default)]
	pub fn new() -> Self {
		CodeParser {
			marks: HashMap::new(),
		}
	}

	pub fn read_func(&mut self, i: &mut In, end: usize) -> Result<Vec<(usize, Insn)>> {
		let start = i.clone();
		let mut ops = Vec::new();
		(|| -> Result<_> {
			self.marks.insert(i.pos(), "\x1B[0;7m[".to_owned());
			while i.pos() < end {
				ops.push((i.pos(), self.read_insn(i)?));
				self.marks.insert(i.pos(), "\x1B[0;7m•".to_owned());
			}
			self.marks.insert(i.pos(), "\x1B[0;7m]".to_owned());
			eyre::ensure!(i.pos() == end, "Overshot: {:X} > {:X}", i.pos(), end);
			Ok(())
		})().map_err(|e| {
			use color_eyre::{Section, SectionExt};
			use std::fmt::Write;
			e.section({
				let mut s = String::new();
				for (addr, op) in &ops {
					writeln!(s, "{:04X}: {:?}", addr, op).unwrap();
				}
				s.pop(); // remove newline
				s.header("Code:")
			}).section({
				start.dump().end(end)
					.marks(self.marks.iter())
					.mark(i.pos()-1, "\x1B[0;7m ")
					.number_width(4)
					.newline(false)
					.to_string()
					.header("Dump:")
			})
		})?;
		Ok(ops)
	}

	fn read_insn(&mut self, i: &mut In) -> Result<Insn> {
		Ok(match i.u8()? {
			0x01 => Insn::Return,
			0x02 => Insn::If(self.read_expr(i)?, i.u16()? as usize),
			0x03 => Insn::Goto(i.u16()? as usize),
			0x04 => Insn::Switch(self.read_expr(i)?, {
				let mut out = Vec::new();
				for _ in 0..i.u16()? {
					out.push((i.u16()?, i.u16()? as usize));
				}
				out
			}, i.u16()? as usize),
			0x08 => Insn::Sleep(i.u32()?),
			0x09 => Insn::FlagsSet(i.u32()?),
			0x0A => Insn::FlagsUnset(i.u32()?),
			0x0B => Insn::FadeOn(i.u32()?, i.u32()?, i.u8()?),
			0x0C => Insn::FadeOff(i.u32()?, i.u32()?),
			0x0D => Insn::_0D,
			0x0F => Insn::Battle(i.u16()?, i.u16()?, i.u16()?, i.u16()?, i.u8()?, i.u16()?, i.i8()?),
			0x16 => Insn::Map(match i.u8()? {
				0x00 => MapInsn::Hide,
				0x01 => MapInsn::Show,
				0x02 => MapInsn::Set(i.i32()?, (i.i32()?, i.i32()?), i.file_ref()?),
				op => eyre::bail!("Unknown MapInsn: {:02X}", op)
			}),
			0x19 => Insn::EventBegin(i.u8()?),
			0x1A => Insn::EventEnd(i.u8()?),
			0x1B => Insn::_1B(i.u16()?, i.u16()?),
			0x1C => Insn::_1C(i.u16()?, i.u16()?),
			0x22 => Insn::SoundPlay(i.u16()?, i.u8()?, i.u8()?),
			0x23 => Insn::SoundStop(i.u16()?),
			0x24 => Insn::SoundLoop(i.u16()?, i.u8()?),
			0x28 => Insn::Quest(i.u16()?, match i.u8()? {
				0x01 => QuestInsn::TaskSet(i.u16()?),
				0x02 => QuestInsn::TaskUnset(i.u16()?),
				0x03 => QuestInsn::FlagsSet(i.u8()?),
				0x04 => QuestInsn::FlagsUnset(i.u8()?),
				op => eyre::bail!("Unknown QuestInsn: {:02X}", op)
			}),
			0x29 => Insn::QuestGet(i.u16()?, match i.u8()? {
				0x00 => QuestGetInsn::Task(i.u16()?),
				0x01 => QuestGetInsn::Flags(i.u8()?),
				op => eyre::bail!("Unknown QuestGetInsn: {:02X}", op)
			}),
			0x30 => Insn::_Party30(i.u8()?),
			0x43 => Insn::CharForkFunc(Character(i.u16()?), i.u8()?, FuncRef(i.u8()? as u16, i.u16()?)),
			0x45 => Insn::CharFork(Character(i.u16()?), i.u16()?, {
				let end = i.u8()? as usize + i.pos();
				let mut insns = Vec::new();
				while i.pos() < end {
					self.marks.insert(i.pos(), "\x1B[0;7;2m•".to_owned());
					insns.push(self.read_insn(i)?);
				}
				eyre::ensure!(i.pos() == end, "Overshot: {:X} > {:X}", i.pos(), end);
				i.check_u8(0)?;
				insns
			}),
			0x49 => Insn::Event(FuncRef(i.u8()? as u16, i.u16()?)),
			0x4D => Insn::ExprVar(i.u16()?, self.read_expr(i)?),
			0x4F => Insn::ExprAttr(i.u8()?, self.read_expr(i)?),
			0x51 => Insn::ExprCharAttr(Character(i.u16()?), i.u8()?, self.read_expr(i)?),
			0x53 => Insn::TextEnd(Character(i.u16()?)),
			0x54 => Insn::TextMessage(self.read_text(i)?),
			0x56 => Insn::TextReset(i.u8()?),
			0x58 => Insn::TextWait,
			0x5A => Insn::TextSetPos(i.i16()?, i.i16()?, i.i16()?, i.i16()?),
			0x5B => Insn::TextTalk(Character(i.u16()?), self.read_text(i)?),
			0x5C => Insn::TextTalkNamed(Character(i.u16()?), i.str()?, self.read_text(i)?),
			0x5D => Insn::Menu(i.u16()?, (i.i16()?, i.i16()?), i.u8()?, i.str()?.split_terminator('\x01').map(|a| a.to_owned()).collect()),
			0x5E => Insn::MenuWait(i.u16()?),
			0x5F => Insn::_Menu5F(i.u16()?),
			0x60 => Insn::TextSetName(i.str()?),
			0x69 => Insn::CamLookAt(Character(i.u16()?), i.u32()?),
			0x6C => Insn::CamAngle(i.i32()?, i.u32()?),
			0x6D => Insn::CamPos(i.pos3()?, i.u32()?),
			0x87 => Insn::CharSetFrame(Character(i.u16()?), i.u16()?),
			0x88 => Insn::CharSetPos(Character(i.u16()?), i.pos3()?, i.u16()?),
			0x8A => Insn::CharLookAt(Character(i.u16()?), Character(i.u16()?), i.u16()?),
			0x8E => Insn::CharWalkTo(Character(i.u16()?), i.pos3()?, i.u32()?, i.u8()?),
			0x90 => Insn::CharWalk(Character(i.u16()?), i.pos3()?, i.u32()?, i.u8()?),
			0x92 => Insn::_Char92(Character(i.u16()?), Character(i.u16()?), i.u32()?, i.u32()?, i.u8()?),
			0x99 => Insn::CharAnimation(Character(i.u16()?), i.u8()?, i.u8()?, i.u32()?),
			0x9A => Insn::CharFlagsSet(Character(i.u16()?), i.u16()?),
			0x9B => Insn::CharFlagsUnset(Character(i.u16()?), i.u16()?),
			0xA2 => Insn::FlagSet(Flag(i.u16()?)),
			0xA3 => Insn::FlagUnset(Flag(i.u16()?)),
			0xA5 => Insn::AwaitFlagUnset(Flag(i.u16()?)),
			0xA6 => Insn::AwaitFlagSet(Flag(i.u16()?)),
			0xB1 => Insn::OpLoad(i.str()?),
			0xB2 => Insn::_B2(i.u8()?, i.u8()?, i.u16()?),
			0xB4 => Insn::ReturnToTitle(i.u8()?),

			op => eyre::bail!("Unknown Insn: {:02X}", op)
		})
	}

	fn read_expr(&mut self, i: &mut In) -> Result<Box<Expr>> {
		#[allow(clippy::vec_box)]
		struct Stack(Vec<Box<Expr>>);
		impl Stack {
			fn push(&mut self, expr: Expr) {
				self.0.push(Box::new(expr))
			}

			fn binop(&mut self, op: ExprBinop) -> Result<Expr> {
				let r = self.pop()?;
				let l = self.pop()?;
				Ok(Expr::Binop(op, l, r))
			}

			fn unop(&mut self, op: ExprUnop) -> Result<Expr> {
				Ok(Expr::Unop(op, self.pop()?))
			}

			fn pop(&mut self) -> Result<Box<Expr>> {
				Ok(self.0.pop().ok_or_else(|| eyre::eyre!("Empty expr stack"))?)
			}
		}
		let mut stack = Stack(Vec::new());
		self.marks.insert(i.pos(), "\x1B[0;7;2m[".to_owned());
		loop {
			let op = match i.u8()? {
				0x00 => Expr::Const(i.u32()?),
				0x01 => break,
				0x02 => stack.binop(ExprBinop::Eq)?,
				0x03 => stack.binop(ExprBinop::Ne)?,
				0x04 => stack.binop(ExprBinop::Lt)?,
				0x05 => stack.binop(ExprBinop::Gt)?,
				0x06 => stack.binop(ExprBinop::Le)?,
				0x07 => stack.binop(ExprBinop::Ge)?,
				0x08 => stack.unop(ExprUnop::Not)?,
				0x09 => stack.binop(ExprBinop::BoolAnd)?,
				0x0A => stack.binop(ExprBinop::And)?,
				0x0B => stack.binop(ExprBinop::Or)?,
				0x0C => stack.binop(ExprBinop::Add)?,
				0x0D => stack.binop(ExprBinop::Sub)?,
				0x0E => stack.unop(ExprUnop::Neg)?,
				0x0F => stack.binop(ExprBinop::Xor)?,
				0x10 => stack.binop(ExprBinop::Mul)?,
				0x11 => stack.binop(ExprBinop::Div)?,
				0x12 => stack.binop(ExprBinop::Mod)?,
				0x13 => stack.unop(ExprUnop::Ass)?,
				0x14 => stack.unop(ExprUnop::MulAss)?,
				0x15 => stack.unop(ExprUnop::DivAss)?,
				0x16 => stack.unop(ExprUnop::ModAss)?,
				0x17 => stack.unop(ExprUnop::AddAss)?,
				0x18 => stack.unop(ExprUnop::SubAss)?,
				0x19 => stack.unop(ExprUnop::AndAss)?,
				0x1A => stack.unop(ExprUnop::XorAss)?,
				0x1B => stack.unop(ExprUnop::OrAss)?,
				0x1C => Expr::Exec(self.read_insn(i)?),
				0x1D => stack.unop(ExprUnop::Inv)?,
				0x1E => Expr::Flag(Flag(i.u16()?)),
				0x1F => Expr::Var(i.u16()?),
				0x20 => Expr::Attr(i.u8()?),
				0x21 => Expr::CharAttr(Character(i.u16()?), i.u8()?),
				0x22 => Expr::Rand,
				op => eyre::bail!("Unknown Expr: {:02X}", op)
			};
			stack.push(op);
			self.marks.insert(i.pos(), "\x1B[0;7;2m•".to_owned());
		}
		self.marks.insert(i.pos(), "\x1B[0;7;2m]".to_owned());
		match stack.0.len() {
			1 => Ok(stack.pop()?),
			_ => eyre::bail!("Invalid Expr: {:?}", stack.0)
		}
	}

	fn read_text(&mut self, i: &mut In) -> Result<Text> {
		self.marks.insert(i.pos(), "\x1B[0;7;2m\"".to_owned());
		let v = util::read_text(i)?;
		self.marks.insert(i.pos(), "\x1B[0;7;2m\"".to_owned());
		Ok(v)
	}
}
