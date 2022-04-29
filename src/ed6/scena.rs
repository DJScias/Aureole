use std::borrow::Cow;
use std::collections::{BTreeSet, BTreeMap};

use derive_more::*;

use choubun::Node;
use kaiseki::ed6::{scena::*, Archives};

pub fn render(scena: &Scena, archives: &Archives, raw: bool) -> choubun::Node {
	ScenaRenderer { scena, archives, raw }.render()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharKind {
	Party,
	Npc,
	Monster,
	Self_,
	Pc,
	Unknown,
}

#[derive(Deref)]
struct ScenaRenderer<'a> {
	#[deref]
	scena: &'a Scena,
	archives: &'a Archives,
	raw: bool,
}


impl ScenaRenderer<'_> {
	fn render(&self) -> Node {
		choubun::document(|doc| {
			let name = format!("{}/{}", self.dir.decode(), self.fname.decode());
			doc.head.node("title", |a| a.text(&name));
			doc.head.node("link", |a| {
				a.attr("rel", "stylesheet");
				a.attr("href", "/assets/style.css"); // XXX url
			});

			doc.body.node("h1", |a| a.text(format!("{} (town: {}, bgm: {})", &name, self.town, self.bgm)));

			doc.body.node("div", |a| {
				a.indent();
				a.attr("id", "chcp");
				a.node("select", |a| {
					a.indent();
					a.attr("id", "ch");
					for &ch in &self.ch {
						a.node("option", |a| a.text(self.file_name(ch)));
					}
				});
				a.node("select", |a| {
					a.indent();
					a.attr("id", "cp");
					for &cp in &self.cp {
						a.node("option", |a| a.text(self.file_name(cp)));
					}
				});
			});

			doc.body.node("h2", |a| a.text("NPCs"));
			doc.body.node("ol", |a| {
				a.indent();
				a.attr("start", 8usize);
				for npc in &self.npcs {
					a.node("li", |a| a.text(format!("{:?}", npc)));
				}
			});

			doc.body.node("h2", |a| a.text("Monsters"));
			doc.body.node("ol", |a| {
				a.indent();
				a.attr("start", 8usize+self.npcs.len());
				for monster in &self.monsters {
					a.node("li", |a| a.text(format!("{:?}", monster)));
				}
			});

			doc.body.node("h2", |a| a.text("Triggers"));
			doc.body.node("ol", |a| {
				a.indent();
				a.attr("start", 0usize);
				for trigger in &self.triggers {
					a.node("li", |a| a.text(format!("{:?}", trigger)));
				}
			});

			doc.body.node("h2", |a| a.text("Object"));
			doc.body.node("ol", |a| {
				a.indent();
				a.attr("start", 0usize);
				for object in &self.objects {
					a.node("li", |a| a.text(format!("{:?}", object)));
				}
			});

			doc.body.node("h2", |a| a.text("Camera angles (?)"));
			doc.body.node("ol", |a| {
				a.indent();
				a.attr("start", 0usize);
				for camera_angle in &self.camera_angles {
					a.node("li", |a| a.text(format!("{:?}", camera_angle)));
				}
			});

			doc.body.node("h2", |a| a.text("Code"));
			for (i, func) in self.functions.iter().enumerate() {
				doc.body.node("h3", |a| a.text(format!("Function {}", i)));
				let render = CodeRenderer { inner: self, indent: 0 };
				if self.raw {
					doc.body.node_class("pre", "code asm", |a| render.asm(a, func));
				} else {
					match decompile(func) {
						Ok(code) => {
							doc.body.node_class("pre", "code", |a| render.code(a, &code));
						},
						Err(e) => {
							tracing::error!("{:?}", e);
							doc.body.node_class("div", "decompile-error", |a| {
								a.text("Decompilation failed. This is a bug.");
							});
							doc.body.node_class("pre", "code asm", |a| render.asm(a, func));
						},
					}
				}
			}
		})
	}

	fn file_name(&self, FileRef(arch, index): FileRef) -> String {
		if let Ok(file) = self.archives.get(arch as u8, index as usize) {
			let name = file.0.name.decode();
			let (prefix, suffix) = name.split_once('.').unwrap_or((&name, ""));
			let prefix = prefix.trim_end_matches(|a| a == ' ');
			let suffix = suffix.trim_start_matches(|a| a == '_');
			format!("{:02}/{}.{}", arch, prefix, suffix)
		} else {
			format!("{:02}/<{}>", arch, index)
		}
	}

	fn char_name(&self, id: usize) -> (CharKind, Cow<str>) {
		let pc_names = &["Estelle", "Joshua", "Scherazard", "Olivier", "Kloe", "Agate", "Tita", "Zin"];

		let npc_start = 8;
		let monster_start = npc_start + self.npcs.len();
		let monster_end = monster_start + self.monsters.len();
		let pc_start = 0x101;
		let pc_end = pc_start + pc_names.len();
		
		fn get_name<T>(idx: usize, items: &[T], f: impl Fn(&T) -> &str) -> Cow<str> {
			let name = f(&items[idx]);
			let mut dups = items.iter().enumerate().filter(|a| f(a.1) == name);
			if dups.clone().count() == 1 {
				name.into()
			} else {
				let dup_idx = dups.position(|a| a.0 == idx).unwrap();
				format!("{} [{}]", name, dup_idx+1).into()
			}
		}

		if id == 0 {
			(CharKind::Party, "[lead]".into())
		} else if (1..4).contains(&id) {
			(CharKind::Party, format!("[party {}]", id+1).into())
		} else if (npc_start..monster_start).contains(&id) {
			(CharKind::Npc, get_name(id-npc_start, &self.npcs, |a| &*a.name))
		} else if (monster_start..monster_end).contains(&id) {
			(CharKind::Monster, get_name(id-monster_start, &self.monsters, |a| &*a.name))
		} else if id == 0xFE {
			(CharKind::Self_, "self".into())
		} else if (pc_start..pc_end).contains(&id) {
			(CharKind::Pc, pc_names[id-pc_start].into())
		} else {
			(CharKind::Unknown, format!("[unknown {}]", id).into())
		}
	}
}

#[extend::ext]
impl Node {
	fn node_class(&mut self, name: &str, class: &str, body: impl FnOnce(&mut Node)) {
		self.node(name, |a| {
			a.class(class);
			body(a);
		})
	}

	fn span(&mut self, class: &str, body: impl FnOnce(&mut Node)) {
		self.node_class("span", class, body)
	}

	fn span_text(&mut self, class: &str, text: impl ToString) {
		self.span(class, |a| a.text(text));
	}
}

#[derive(Deref)]
struct CodeRenderer<'a> {
	#[deref]
	inner: &'a ScenaRenderer<'a>,
	indent: u32,
}

impl<'a> CodeRenderer<'a> {
	fn indent(&self) -> Self {
		CodeRenderer { inner: self.inner, indent: self.indent + 1 }
	}

	fn asm(&self, a: &mut Node, asm: &Asm) {
		let mut labels = BTreeSet::<usize>::new();
		for (_, insn) in &asm.code {
			insn.labels(|a| { labels.insert(a); });
		}

		let labels: BTreeMap<usize, String> =
			labels.into_iter()
			.enumerate()
			.map(|(i, a)| (a, format!("L{}", i)))
			.collect();

		let render_label = |a: &mut Node, addr: usize| {
			a.span("label", |a| {
				a.attr("title", addr);
				a.text(&labels[&addr]);
			});
		};

		for (addr, insn) in &asm.code {
			if labels.contains_key(addr) {
				render_label(a, *addr);
				a.span_text("syntax", ":");
				a.text("\n");
			}
			a.text("  ");

			match insn {
				FlowInsn::If(expr, target) => {
					a.span_text("keyword", "UNLESS");
					a.text(" ");
					self.expr(a, expr);
					a.text(" ");
					a.span_text("keyword", "GOTO");
					a.text(" ");
					render_label(a, *target);
				}

				FlowInsn::Goto(target) => {
					a.span_text("keyword", "GOTO");
					a.text(" ");
					render_label(a, *target);
				}

				FlowInsn::Switch(expr, branches, default) => {
					a.span_text("keyword", "SWITCH");
					a.text(" ");
					self.expr(a, expr);
					a.text(" ");
					a.span_text("syntax", "[");
					for (case, target) in branches {
						a.span_text("case", case);
						a.span_text("syntax", ":");
						a.text(" ");
						render_label(a, *target);
						a.span_text("syntax", ",");
						a.text(" ");
					}
					a.span_text("keyword", "default");
					a.span_text("syntax", ":");
					a.text(" ");
					render_label(a, *default);
					a.span_text("syntax", "]");
				}

				FlowInsn::Insn(insn) => {
					self.insn(a, insn).end();
				}
			}
			a.text("\n");
		}
	}

	fn line(&self, a: &mut Node) {
		for _ in 0..self.indent {
			a.span_text("indent", "\t");
		}
	}

	fn code(&self, a: &mut Node, code: &[Stmt]) {
		if code.is_empty() {
			self.line(a);
			a.span_text("empty-block", "(empty)");
			a.text("\n");
		}

		for stmt in code {
			match stmt {
				Stmt::If(cases) => {
					self.line(a);
					a.span_text("keyword", "IF");
					a.text("\n");

					let inner = self.indent();
					for (expr, body) in cases {
						inner.line(a);
						match expr {
							Some(expr) => self.expr(a, expr),
							None => a.span_text("keyword", "ELSE"),
						}
						a.text(" ");
						a.span_text("syntax", "=>");
						a.text("\n");

						inner.indent().code(a, body);
					}
				}

				Stmt::Switch(expr, cases) => {
					self.line(a);
					a.span_text("keyword", "SWITCH");
					a.text(" ");
					self.expr(a, expr);
					a.text("\n");

					let inner = self.indent();
					for (cases, body) in cases {
						inner.line(a);
						let mut first = true;
						for case in cases {
							if !first {
								a.span_text("syntax", ",");
								a.text(" ");
							}
							first = false;
							match case {
								Some(case) => a.span_text("case", case),
								None => a.span_text("keyword", "default"),
							}
						}
						a.text(" ");
						a.span_text("syntax", "=>");
						a.text("\n");

						inner.indent().code(a, body);
					}
				}

				Stmt::While(expr, body) => {
					self.line(a);
					a.span_text("keyword", "WHILE");
					a.text(" ");
					self.expr(a, expr);
					a.text("\n");

					self.indent().code(a, body);
				}

				Stmt::Break => {
					self.line(a);
					a.span_text("keyword", "BREAK");
					a.text("\n");
				}

				Stmt::Insn(insn) => {
					self.line(a);
					self.insn(a, insn).end();
				}
			}
		}
	}

	fn expr(&self, a: &mut Node, expr: &Expr) {
		self.expr_inner(a, expr, 0)
	}

	fn expr_inner(&self, a: &mut Node, expr: &Expr, prio: u8) {
		match expr {
			Expr::Const(v) => {
				a.span_text("int", v);
			}

			Expr::Binop(op, l, r) => {
				let (text, prio2) = match op {
					ExprBinop::Eq      => ("==", 4),
					ExprBinop::Ne      => ("!=", 4),
					ExprBinop::Lt      => ("<",  4),
					ExprBinop::Gt      => (">",  4),
					ExprBinop::Le      => ("<=", 4),
					ExprBinop::Ge      => (">=", 4),
					ExprBinop::BoolAnd => ("&&", 3),
					ExprBinop::And     => ("&", 3),
					ExprBinop::Or      => ("|", 1),
					ExprBinop::Add     => ("+", 5),
					ExprBinop::Sub     => ("-", 5),
					ExprBinop::Xor     => ("^", 2),
					ExprBinop::Mul     => ("*", 6),
					ExprBinop::Div     => ("/", 6),
					ExprBinop::Mod     => ("%", 6),
				};
				if prio2 < prio || self.raw { a.span_text("syntax", "("); }
				self.expr_inner(a, l, prio2);
				a.text(" ");
				a.span_text("expr-op", text);
				a.text(" ");
				self.expr_inner(a, r, prio2+1);
				if prio2 < prio || self.raw { a.span_text("syntax", ")"); }
			}

			Expr::Unop(op, v) => {
				let (text, is_assign) = match op {
					ExprUnop::Not    => ("!", false),
					ExprUnop::Neg    => ("-", false),
					ExprUnop::Inv    => ("~", false),
					ExprUnop::Ass    => ("=",  true),
					ExprUnop::MulAss => ("*=", true),
					ExprUnop::DivAss => ("/=", true),
					ExprUnop::ModAss => ("%=", true),
					ExprUnop::AddAss => ("+=", true),
					ExprUnop::SubAss => ("-=", true),
					ExprUnop::AndAss => ("&=", true),
					ExprUnop::XorAss => ("^=", true),
					ExprUnop::OrAss  => ("|=", true),
				};
				a.span_text("expr-op", text);
				if is_assign {
					a.text(" ");
					self.expr_inner(a, v, 0);
				} else {
					self.expr_inner(a, v, 100);
				}
			}

			Expr::Exec(insn) => {
				self.insn(a, insn);
			}
			Expr::Flag(flag) => {
				let mut r = self.visitor(a, "Flag");
				r.accept(InsnArg::flag(flag));
			}
			Expr::Var(var) => {
				let mut r = self.visitor(a, "Var");
				r.accept(InsnArg::var(var));
			}
			Expr::Attr(attr) => {
				let mut r = self.visitor(a, "Attr");
				r.accept(InsnArg::attr(attr));
			}
			Expr::CharAttr(char, attr) => {
				let mut r = self.visitor(a, "CharAttr");
				r.accept(InsnArg::char(char));
				r.accept(InsnArg::char_attr(attr));
			},
			Expr::Rand => {
				self.visitor(a, "Rand");
			}
		}
	}

	fn visitor<'b>(&self, a: &'b mut Node, name: &'static str) -> InsnRenderer<'a, 'b> {
		a.span_text("insn", name);
		InsnRenderer { inner: self.indent(), node: a, is_block: false }
	}

	fn insn<'b>(&self, a: &'b mut Node, insn: &Insn) -> InsnRenderer<'a, 'b> {
		let (name, args) = insn.parts();
		let mut vis = self.visitor(a, name);
		for arg in args.iter() {
			vis.accept(*arg)
		}
		vis
	}
}

#[derive(Deref)]
struct InsnRenderer<'a, 'b> {
	#[deref]
	inner: CodeRenderer<'a>,
	node: &'b mut Node,
	is_block: bool,
}

impl InsnRenderer<'_, '_> {
	fn end(&mut self) {
		if !self.is_block {
			self.node.text("\n");
		}
		self.is_block = true;
	}

	fn accept(&mut self, arg: InsnArg) {
		match arg {
			InsnArg::u8(v) => { self.node.text(" "); self.node.span_text("int", v); }
			InsnArg::u16(v) => { self.node.text(" "); self.node.span_text("int", v); }
			InsnArg::u32(v) => { self.node.text(" "); self.node.span_text("int", v); }

			InsnArg::i8(v) => { self.node.text(" "); self.node.span_text("int", v); }
			InsnArg::i16(v) => { self.node.text(" "); self.node.span_text("int", v); }
			InsnArg::i32(v) => { self.node.text(" "); self.node.span_text("int", v); }

			InsnArg::scena_file(v) => {
				self.node.text(" ");
				let text = self.file_name(*v);
				if text.get(2..3) == Some("/") && text.ends_with(".SN") {
					self.node.node("a", |a| {
						a.class("file-ref");
						a.attr("href", &text[3..text.len()-3]); // XXX url
						a.text(text);
					});
				} else {
					self.node.span_text("file-ref", text);
				}
			}

			InsnArg::map_file(v) => {
				self.node.text(" ");
				self.node.span_text("file-ref", self.file_name(*v));
			}
			InsnArg::vis_file(v) => {
				self.node.text(" ");
				self.node.span_text("file-ref", self.file_name(*v));
			}
			InsnArg::eff_file(v) => {
				self.node.text(" ");
				self.node.span_text("file-ref", v);
			}
			InsnArg::op_file(v) => {
				self.node.text(" ");
				self.node.span_text("file-ref", v);
			}
			InsnArg::avi_file(v) => {
				self.node.text(" ");
				self.node.span_text("file-ref", v);
			}

			InsnArg::pos2(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::pos3(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::relative(v) => { self.node.text(" "); self.node.span_text("unknown", format!("relative{:?}", v)); }

			InsnArg::time(v) => { self.node.text(" "); self.node.span_text("time", format!("{}ms", v)); }
			InsnArg::speed(v) => { self.node.text(" "); self.node.span_text("speed", format!("{}mm/s", v)); }
			InsnArg::angle(v) => { self.node.text(" "); self.node.span_text("angle", format!("{}°", v)); }
			InsnArg::color(v) => {
				self.node.text(" ");
				self.node.span("color", |a| {
					a.attr("style", format!("--splat-color: #{:06X}; --splat-alpha: {}", v&0xFFFFFF, (v>>24) as f32 / 255.0));
					a.node_class("svg", "color-splat", |a| a.node("use", |a| a.attr("href", "/assets/color-splat.svg#splat"))); // XXX url
					a.text(format!("#{:08X}", v));
				});
			}

			InsnArg::time16(v) => { self.node.text(" "); self.node.span_text("time", format!("{}ms", v)); }
			InsnArg::angle32(v) => { self.node.text(" "); self.node.span_text("angle", format!("{}m°", v)); }

			InsnArg::battle(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::town(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::bgmtbl(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::quest(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::sound(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::item(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::flag(v) => {
				self.node.text(" ");
				self.node.span_text("flag", v);
			}
			InsnArg::shop(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::magic(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }

			InsnArg::fork(v) => {
				if self.inner.raw {
					self.node.text(" ");
					self.node.span_text("syntax", "[");
					self.node.text("\n");
				}

				for insn in v {
					self.inner.line(self.node);
					self.inner.insn(self.node, insn).end();
				}

				if self.inner.raw {
					self.inner.line(self.node);
					self.node.span_text("syntax", "]");
				}
			}

			InsnArg::expr(v) => {
				self.node.text(" ");
				self.inner.expr(self.node, v);
			}

			InsnArg::string(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::text(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }

			InsnArg::menu(v) => {
				self.end();
				self.node.node("div", |a| {
					a.attr("role", "list");
					a.class("block menu");
					a.attr("style", format!("--indent: {}", self.inner.indent));
					for (idx, line) in v.iter().enumerate() {
						a.node("div", |a| {
							a.class("menu-row");
							a.span_text("menu-idx", format!("({})", idx));
							a.text(" ");
							a.span("menu-label", |a| {
								a.attr("role", "listitem");
								a.text(line);
							});
							a.text("\n");
						});
					}
				});
			}

			InsnArg::quests(v) => {
				for q in v {
					self.accept(InsnArg::quest(q))
				}
			}

			InsnArg::emote(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }

			InsnArg::flags(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::quest_flag(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::char_flags(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::quest_task(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::member(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::element(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }

			InsnArg::var(v) => {
				self.node.text(" ");
				self.node.span_text("var", v);
			}
			InsnArg::attr(v) => {
				self.node.text(" ");
				self.node.span_text("attr", v);
			}

			InsnArg::char_attr(v) => {
				self.node.span("char-attr", |a| {
					a.text(":");

					let name = match *v {
						1 => Some("x"),
						2 => Some("y"),
						3 => Some("z"),
						4 => Some("angle"),
						_ => None,
					};
					match name {
						Some(name) => a.node("span", |a| {
							if self.inner.raw {
								a.attr("title", name);
								a.text(v);
							} else {
								a.attr("title", v);
								a.text(name);
							}
						}),
						None => a.text(v),
					}
				});
			}

			InsnArg::char(v) => {
				self.node.text(" ");

				let (kind, name) = self.char_name(*v as usize);
				let kind = match kind {
					CharKind::Party => "party",
					CharKind::Npc => "npc",
					CharKind::Monster => "monster",
					CharKind::Self_ => "self",
					CharKind::Pc => "pc",
					CharKind::Unknown => "unknown",
				};
				self.node.span("char", |a| {
					a.class(&format!("char-{}", kind));
					if self.inner.raw {
						a.attr("title", format!("{} ({})", name, kind));
						a.text(v);
					} else {
						a.attr("title", format!("{} ({})", v, kind));
						a.text(name);
					}
				});
			}

			InsnArg::chcp(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::fork_id(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::menu_id(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::object(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
			InsnArg::func_ref(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }

			InsnArg::data(v) => { self.node.text(" "); self.node.span_text("unknown", format!("{:?}", v)); }
		}
	}
}
