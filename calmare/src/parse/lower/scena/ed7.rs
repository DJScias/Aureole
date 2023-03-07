use super::*;

use themelios::scena::{ed7::*, code::Bytecode};

themelios::util::newtype!(CharDefId, u16);
newtype!(CharDefId, "char");

themelios::util::newtype!(FuncId, u16);
newtype!(FuncId, "fn");

newtype!(SepithId, "sepith");
newtype!(AtRollId, "at_roll");
newtype!(PlacementId, "placement");

#[derive(Debug, Clone)]
pub struct Header {
	pub name: (String, String, String),
	pub town: TownId,
	pub bgm: BgmId,
	pub flags: u32,
	pub unk: (u8, u16, u8),
	pub scp: [FileId; 6],
}

#[derive(Debug, Clone)]
pub enum NpcOrMonster {
	Npc(Npc),
	Monster(Monster),
}

struct FPos3(f32, f32, f32);
impl Val for FPos3 {
	fn parse(p: &mut Parse) -> Result<Self> {
		if let Some((x, y, z)) = p.tuple()? {
			Ok(FPos3(x, y, z))
		} else {
			Diag::error(p.next_span(), "expected fpos3").emit();
			Err(Error)
		}
	}
}

#[derive(Debug, Clone, Default)]
struct ScenaBuild {
	header: One<Header>,
	entry: One<Entry>,
	chcp: Many<ChcpId, FileId>,
	npcs_monsters: Many<CharDefId, NpcOrMonster>,
	triggers: Many<TriggerId, Trigger>,
	look_points: Many<LookPointId, LookPoint>,
	labels: Many<LabelId, Label>,
	animations: Many<AnimId, Animation>,
	sepith: Many<SepithId, [u8; 8]>,
	at_rolls: Many<AtRollId, [u8; 16]>,
	placements: Many<PlacementId, [(u8, u8, Angle); 8]>,
	battles: Many<BattleId, Battle>,
	functions: Many<FuncId, Bytecode>,
}

pub fn parse(lines: &[Line], ctx: &Context) -> Result<Scena> {
	let mut scena = ScenaBuild::default();
	for line in lines {
		let _ = Parse::new(line, ctx).parse_with(|p| parse_line(&mut scena, p));
	}

	let misorder = scena.npcs_monsters.0.iter()
		.skip_while(|a| !matches!(&a.1.1, Some(NpcOrMonster::Monster(_))))
		.find(|a| matches!(&a.1.1, Some(NpcOrMonster::Npc(_))));
	if let Some((k, S(s, _))) = misorder {
		let (_, S(prev, _)) = scena.npcs_monsters.0.range(..k).last().unwrap();
		Diag::error(*prev, "monsters must come after npcs")
			.note(*s, "is before this npc")
			.emit();
	}

	let mut npcs = Vec::new();
	let mut monsters = Vec::new();
	for m in scena.npcs_monsters.get(|a| a.0 as usize) {
		match m {
			NpcOrMonster::Npc(n) => npcs.push(n),
			NpcOrMonster::Monster(m) => monsters.push(m),
		}
	}

	if !scena.header.is_present() {
		Diag::error(Span::new_at(0), "missing 'scena' block").emit();
	}

	let chcp = scena.chcp.get(|a| a.0 as usize);
	let labels = Some(scena.labels.get(|a| a.0 as usize));
	let triggers = scena.triggers.get(|a| a.0 as usize);
	let look_points = scena.look_points.get(|a| a.0 as usize);
	let animations = scena.animations.get(|a| a.0 as usize);
	let entry = scena.entry.get();
	let functions = scena.functions.get(|a| a.0 as usize);
	let sepith = scena.sepith.get(|a| a.0 as usize);
	let at_rolls = scena.at_rolls.get(|a| a.0 as usize);
	let placements = scena.placements.get(|a| a.0 as usize);
	let battles = scena.battles.get(|a| a.0 as usize);

	let h = scena.header.get().ok_or(Error)?;

	Ok(Scena {
		name1: h.name.0,
		name2: h.name.1,
		filename: h.name.2,
		town: h.town,
		bgm: h.bgm,
		flags: h.flags,
		includes: h.scp,
		chcp,
		labels,
		npcs,
		monsters,
		triggers,
		look_points,
		animations,
		entry,
		functions,
		sepith,
		at_rolls,
		placements,
		battles,
		unk1: h.unk.0,
		unk2: h.unk.1,
		unk3: h.unk.2,
	})
}

fn parse_line(scena: &mut ScenaBuild, p: &mut Parse) -> Result<()> {
	let Some(key) = p.next_if(f!(Token::Ident(a) => a)) else {
		Diag::error(p.next_span(), "expected word").emit();
		p.pos = p.tokens.len();
		return Err(Error);
	};
	if p.next_if(f!(Token::Bracket(_) => ())).is_some() {
		p.pos -= 2;
	}
	match *key {
		"fn" => {
			let S(s, n) = Val::parse(p)?;
			scena.functions.mark(p.tokens[0].0 | s, n);
			let f = parse_func(p);
			scena.functions.insert(n, f);
		}
		"scena" => {
			scena.header.mark(p.head_span());
			let mut scp = <[One<FileId>; 6]>::default();
			parse_data!(p => {
				name,
				town,
				bgm,
				flags,
				unk,
				scp => |p: &mut Parse| {
					let S(s, n) = Val::parse(p)?;
					let n: u32 = n;
					if n >= 6 {
						Diag::error(s, "only values 0-5 allowed").emit();
						return Err(Error)
					}
					scp[n as usize].mark(p.tokens[0].0 | s);
					let v = Val::parse(p)?;
					scp[n as usize].set(v);
					Ok(())
				}
			});
			let scp = scp.map(|a| a.get().unwrap_or(FileId(0)));
			scena.header.set(Header { name, town, bgm, flags, unk, scp });
		}
		"entry" => {
			scena.entry.mark(p.tokens[0].0);
			parse_data!(p => {
				pos,
				unk1,
				cam_from,
				cam_pers,
				unk2,
				cam_deg,
				cam_limit,
				cam_at,
				unk3,
				unk4,
				flags,
				town,
				init,
				reinit,
			});
			scena.entry.set(Entry {
				pos, unk1, cam_from, cam_pers, unk2, cam_deg, cam_limit,
				cam_at, unk3, unk4, flags, town, init, reinit,
			});
		}
		"chcp" => {
			let (S(s, n), v) = Val::parse(p)?;
			scena.chcp.mark(p.tokens[0].0 | s, n);
			scena.chcp.insert(n, v);
		}
		"npc" => {
			let S(s, n) = Val::parse(p)?;
			scena.npcs_monsters.mark(p.tokens[0].0 | s, n);
			parse_data!(p => {
				name,
				pos,
				angle,
				flags,
				unk2,
				chcp,
				init,
				talk,
				unk4,
			});
			scena.npcs_monsters.insert(n, NpcOrMonster::Npc(Npc {
				name, pos, angle, flags, unk2,
				chcp, init, talk, unk4,
			}));
		}
		"monster" => {
			let S(s, n) = Val::parse(p)?;
			scena.npcs_monsters.mark(p.tokens[0].0 | s, n);
			parse_data!(p => {
				pos,
				angle,
				flags,
				battle,
				flag,
				chcp,
				unk2,
				stand_anim,
				walk_anim,
			});
			scena.npcs_monsters.insert(n, NpcOrMonster::Monster(Monster {
				pos, angle, flags, battle, flag,
				chcp, unk2, stand_anim, walk_anim,
			}));
		}
		"trigger" => {
			let S(s, n) = Val::parse(p)?;
			scena.triggers.mark(p.tokens[0].0 | s, n);
			parse_data!(p => {
				pos,
				radius,
				transform,
				unk1,
				unk2,
				function,
				unk3,
				unk4,
				unk5,
				unk6,
			});
			let FPos3(x, y, z) = pos;
			let radius: f32 = radius;
			scena.triggers.insert(n, Trigger {
				pos: (x / 1000., y / 1000., z / 1000.),
				radius: radius / 1000.,
				transform,
				unk1,
				unk2,
				function,
				unk3,
				unk4,
				unk5,
				unk6,
			});
		}
		"look_point" => {
			let S(s, n) = Val::parse(p)?;
			scena.look_points.mark(p.tokens[0].0 | s, n);
			parse_data!(p => {
				pos,
				radius,
				bubble_pos,
				unk1,
				unk2,
				function,
				unk3,
				unk4,
			});
			scena.look_points.insert(n, LookPoint {
				pos,
				radius,
				bubble_pos,
				unk1,
				unk2,
				function,
				unk3,
				unk4,
			});
		}
		"label" => {
			let S(s, n) = Val::parse(p)?;
			scena.labels.mark(p.tokens[0].0 | s, n);
			parse_data!(p => {
				name,
				pos,
				unk1,
				unk2,
			});
			let FPos3(x, y, z) = pos;
			scena.labels.insert(n, Label {
				name,
				pos: (x / 1000., y / 1000., z / 1000.),
				unk1,
				unk2,
			});
		}
		"anim" => {
			let (S(s, n), speed, frames) = Val::parse(p)?;
			scena.animations.mark(p.tokens[0].0 | s, n);
			scena.animations.insert(n, Animation {
				speed,
				frames,
			});
		}
		"sepith" => {
			let (S(s, n), values) = Val::parse(p)?;
			scena.sepith.mark(p.tokens[0].0 | s, n);
			scena.sepith.insert(n, values);
		}
		"at_roll" => {
			let S(s, n) = Val::parse(p)?;
			scena.at_rolls.mark(p.tokens[0].0 | s, n);
			let mut values = <[One::<u8>; 16]>::default();
			macro fd($n:literal) {
				|p: &mut Parse| {
					values[$n].mark(p.tokens[0].0);
					values[$n].set(Val::parse(p)?);
					Ok(())
				}
			}
			parse_data!(p => {
				none => fd!(0),
				hp10 => fd!(1),
				hp50 => fd!(2),
				ep10 => fd!(3),
				ep50 => fd!(4),
				cp10 => fd!(5),
				cp50 => fd!(6),
				unk1 => fd!(7),
				unk2 => fd!(8),
				unk3 => fd!(9),
				unk4 => fd!(10),
				unk5 => fd!(11),
				unk6 => fd!(12),
				unk7 => fd!(13),
				unk8 => fd!(14),
				unk9 => fd!(15),
			});
			let values = values.map(|a| a.get().unwrap_or_default());
			scena.at_rolls.insert(n, values);
		}
		"placement" => {
			let S(s, n) = Val::parse(p)?;
			scena.placements.mark(p.tokens[0].0 | s, n);
			let mut vs = Vec::new();
			parse_data!(p => {
				pos => |p: &mut Parse| {
					vs.push(Val::parse(p)?);
					Ok(())
				}
			});
			if let Ok(vs) = vs.try_into() {
				scena.placements.insert(n, vs);
			} else {
				Diag::error(p.head_span(), "needs exactly 8 'pos'").emit();
			}
		}
		"battle" => {
			let S(s, n) = Val::parse(p)?;
			scena.battles.mark(p.tokens[0].0 | s, n);
			let mut setups = Vec::new();
			parse_data!(p => {
				flags, level, unk1, vision_range, move_range,
				can_move, move_speed, unk2, battlefield, sepith,
				setup => |p: &mut Parse| {
					let weight = Val::parse(p)?;
					parse_data!(p => {
						enemies, placement, bgm, at_roll
					});
					let (placement, placement_ambush) = placement;
					let (bgm, bgm_ambush) = bgm;
					if setups.len() >= 4 {
						Diag::error(p.head_span(), "only up to 4 setups allowed").emit();
						return Err(Error)
					}
					setups.push(BattleSetup {
						weight,
						enemies,
						placement,
						placement_ambush,
						bgm,
						bgm_ambush,
						at_roll,
					});
					Ok(())
				}
			});
			scena.battles.insert(n, Battle {
				flags, level, unk1, vision_range, move_range,
				can_move, move_speed, unk2, battlefield, sepith,
				setups,
			});
		}
		_ => {
			Diag::error(p.tokens[0].0, "unknown declaration")
				.note(p.tokens[0].0, "expected \
					'scena', 'entry', 'chcp', 'npc', 'monster', \
					'trigger', 'look_point', 'label', 'anim', \
					'sepith', 'at_roll', 'placement', 'battle', 'fn'")
				.emit();
		}
	}
	Ok(())
}
