use hamu::read::le::*;
use hamu::write::le::*;
use crate::types::Time;
use crate::util::*;

pub mod code;
pub mod ed6;
pub mod ed7;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(derive_more::DebugCustom)]
#[debug(fmt = "FuncId({_0}, {_1})")]
pub struct FuncId(pub u16, pub u16);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(derive_more::DebugCustom)]
#[debug(fmt = "Pos2({_0}, {_1})")]
pub struct Pos2(pub i32, pub i32);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(derive_more::DebugCustom)]
#[debug(fmt = "Pos3({_0}, {_1}, {_2})")]
pub struct Pos3(pub i32, pub i32, pub i32);

// 0x00000001 SF_CAMERA_AUTO
// 0x00400000 SF_ENTRY_DISABLE
// 0x02000000 SF_FADEBGM_DISABLE
newtype!(SystemFlags, u32);

// 0x10 done
newtype!(QuestFlags, u8);
newtype!(QuestTask, u16);

// 0x0002 PF_NOVEC
// 0x0004 PF_NOHEIGHT
// 0x0008 PF_NODISP
// 0x0010 PF_NOTURN
// 0x0020 PF_NOANIME
// 0x0040 PF_NOATARI
// 0x0080 PF_UNDEF
newtype!(CharFlags, u16);

// 0x0004 MOF_NODISP
// 0x0020 MOF_LOOPPLAY
newtype!(ObjectFlags, u32);

newtype!(LookPointFlags, u16);
newtype!(EntryFlags, u16);
newtype!(TriggerFlags, u16);

// A bitfield in 3rd's party select menu
newtype!(TcMembers, u32);

newtype!(Var, u16); // called Work internally

newtype!(Global, u8);

// 0 SW_ENTRY_NO
// 1 SW_BGM_NO
// 3 battle result
// 4 current chapter
// 10 party lead
// 11 party second
// 12 party third
// 13 party fourth
// 14 party fifth (guest)
// 15 party sixth (guest)
// 18 current mira
// 19 ItemId used in item handler
// 21 number of battles
// 26 used much during the Madrigal, and when Joshua is activated in SC.
// 27 Boolean. Often set together with 26.
// 35 set to 255 once after rescuing Tita in FC
// 28 Boolean. Generally only set to true for a short time
// 40 SW_CURSOR_FORM (24 MSCRS_NORMAL, FFFF MSCRS_VOID)
// 41 BattleId, generally checked in reinit
// 42 sometimes set to a negative number in reinit. I suspect it's something with altitude.
// 43 set to 255 in some reinit in FC
// 45 SW_MOVIE_STATE
// 46 CharId, set together with 26 and 27 in the Madrogal. Spotlight character?
// 47 Bracer rank
// 49 TownId for the next save, values include 19, 140, 302, 400, 401, 274, 259, 297, 296, 299
newtype!(Attr, u8);

newtype!(ChipId, u16);
newtype!(CharId, u16);
newtype!(LookPointId, u16);
newtype!(ObjectId, u16);
newtype!(EntranceId, u8);
newtype!(TriggerId, u16);
newtype!(LabelId, u16);
newtype!(AnimId, u16);

newtype!(EffId, u8);
newtype!(EffInstanceId, u8);
newtype!(MenuId, u16);
newtype!(VisId, u8);
newtype!(ForkId, u16);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[derive(derive_more::DebugCustom)]
#[debug(fmt = "CharAttr({_0:?}, {_1})")]
pub struct CharAttr(pub CharId, pub u8);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[derive(derive_more::DebugCustom)]
#[debug(fmt = "Emote({_0:?}, {_1}, {_2:?}, {_3})")]
pub struct Emote(pub u8, pub u8, pub Time, pub u8);

pub trait ReadStreamExt2: ReadStream {
	fn pos2(&mut self) -> Result<Pos2, Self::Error> {
		Ok(Pos2(self.i32()?, self.i32()?))
	}

	fn pos3(&mut self) -> Result<Pos3, Self::Error> {
		Ok(Pos3(self.i32()?, self.i32()?, self.i32()?))
	}
}
impl<T: ReadStream> ReadStreamExt2 for T {}

pub trait WriteStreamExt2: WriteStream {
	fn pos2(&mut self, p: Pos2) {
		self.i32(p.0);
		self.i32(p.1);
	}

	fn pos3(&mut self, p: Pos3) {
		self.i32(p.0);
		self.i32(p.1);
		self.i32(p.2);
	}
}
impl<T: WriteStream> WriteStreamExt2 for T {}
