use themelios::scena::code::{Bytecode, Insn, Expr, ExprBinop, ExprUnop};
use themelios::scena::code::decompile::{recompile, TreeInsn};

use super::*;
use crate::span::{Spanned as S, Span};

pub mod ed7;

fn parse_func(p: &mut Parse) -> Bytecode {
	let tree = parse_tree(p, false, false);
	recompile(&tree).map_err(|e| {
		Diag::error(p.head_span(), "unknown recompile error")
			.note(p.head_span(), e)
			.emit();
		Error
	}).unwrap_or_default()
}

impl Val for Vec<Insn> {
	fn parse(p: &mut Parse) -> Result<Self> {
		let mut out = Vec::new();
		for line in p.body()? {
			out.push(Parse::new(line, p.context).parse_with(parse_insn));
		}
		Ok(out)
	}
}

fn parse_tree(p: &mut Parse, can_break: bool, can_continue: bool) -> Vec<TreeInsn> {
	let mut out = Vec::new();
	let mut last_if = None;
	for l in p.body().unwrap_or_default() {
		let p = &mut Parse::new(l, p.context);

		let span = p.next_span();
		let key = test!(p, Token::Ident(a) => *a).unwrap_or_default();

		match key {
			"if" => {
				let e = parse_expr(p);
				let b = parse_tree(p, can_break, can_continue);
				out.push(TreeInsn::If(vec![(Some(e), b)]));
				let TreeInsn::If(a) = out.last_mut().unwrap() else { unreachable!() };
				last_if = Some(a);
			}

			"elif" => {
				let e = parse_expr(p);
				let b = parse_tree(p, can_break, can_continue);
				if let Some(a) = last_if {
					a.push((Some(e), b));
					last_if = Some(a)
				} else {
					Diag::error(span, "unexpected elif").emit();
				}
			},

			"else" => {
				let b = parse_tree(p, can_break, can_continue);
				if let Some(a) = last_if {
					a.push((None, b));
					last_if = None;
				} else {
					Diag::error(span, "unexpected else").emit();
				}
			}

			"while" => {
				last_if = None;
				let e = parse_expr(p);
				let b = parse_tree(p, true, true);
				out.push(TreeInsn::While(e, b));
			}

			"switch" => {
				last_if = None;
				let e = parse_expr(p);
				let mut cases = Vec::new();
				let mut seen = Many::<Option<u16>, ()>::default(); // only used for duplicate checking, not order
				for l in p.body().unwrap_or_default() {
					Parse::new(l, p.context).parse_with(|p| {
						let span = p.next_span();
						let key = test!(p, Token::Ident(a) => *a).unwrap_or_default();
						let i = match key {
							"case" => u16::parse(p).map(Some),
							"default" => Ok(None),
							_ => {
								Diag::error(span, "expected 'case' or 'default'").emit();
								Err(Error)
							}
						};
						let b = parse_tree(p, true, can_continue);
						if let Ok(i) = i {
							seen.mark(span, i);
							cases.push((i, b))
						}
					});
				}
				out.push(TreeInsn::Switch(e, cases));
			}

			"break" => {
				last_if = None;
				if can_break {
					out.push(TreeInsn::Break);
				} else {
					Diag::error(span, "can't break here").emit();
				}
			}

			"continue" => {
				last_if = None;
				if can_continue {
					out.push(TreeInsn::Continue);
				} else {
					Diag::error(span, "can't continue here").emit();
				}
			}

			_ => {
				p.pos -= 1;
				last_if = None;
				let insn = parse_insn(p);
				out.push(TreeInsn::Insn(insn));
			}
		}
		p.finish();
	}
	out
}

fn parse_insn(p: &mut Parse) -> Insn {
	if p.pos == p.tokens.len() {
		Diag::error(p.next_span(), "can't parse insn").emit();
		return Insn::Return()
	}
	macro run {
		([$(($ident:ident $(($_n:ident $($ty:tt)*))*))*]) => {
			match p.tokens[p.pos].1 {
				$(Token::Ident(stringify!($ident)) => {
					p.pos += 1;
					run!($ident $(($_n $($ty)*))*);
				})*
				_ => return parse_assignment(p)
			}
		},
		($ident:ident ($v1:ident $_:ty) ($v2:ident Expr)) => {
			Diag::error(p.prev_span(), "please use assignment syntax").emit();
			p.pos = p.tokens.len();
			return Insn::Return()
		},
		($ident:ident $(($_n:ident $ty:ty))*) => {
			if let Ok::<Insn, Error>(i) = try {
				Insn::$ident($(<$ty>::parse(p)?),*)
			} {
				return i
			} else {
				p.pos = p.tokens.len();
				return Insn::Return()
			}
		}
	}
	themelios::scena::code::introspect!(run);
}

fn parse_assignment(p: &mut Parse) -> Insn {
	macro run {
		([$(($ident:ident $(($_n:ident $($ty:tt)*))*))*]) => {
			$(run!($ident $(($_n $($ty)*))*);)*
		},
		($ident:ident ($v1:ident $t:ty) ($v2:ident Expr)) => {
			match <$t>::try_parse(p) {
				Ok(Some(a)) => {
					let e = parse_assignment_expr(p);
					return Insn::$ident(a, e);
				}
				Ok(None) => {}
				Err(_) => {
					p.pos = p.tokens.len();
					return Insn::Return()
				}
			}
		},
		($ident:ident $($t:tt)*) => {}
	}
	themelios::scena::code::introspect!(run);

	Diag::error(p.next_span(), "can't parse insn").emit();
	p.pos = p.tokens.len();
	Insn::Return()
}

fn parse_expr(p: &mut Parse) -> Expr {
	parse_expr0(p, 10).unwrap_or(Expr::Const(0))
}

fn parse_assignment_expr(p: &mut Parse) -> Expr {
	let op = parse_assop(p).unwrap_or_else(|| {
		Diag::error(p.next_span(), "expected assignment operator").emit();
		ExprUnop::Ass
	});
	let e = parse_expr(p);
	Expr::Unop(op, Box::new(e))
}

fn parse_expr0(p: &mut Parse, prec: usize) -> Result<Expr> {
	let mut e = parse_atom(p)?;
	while let Some(op) = parse_binop(p, prec) {
		let e2 = parse_expr0(p, prec-1)?;
		e = Expr::Binop(op, Box::new(e), Box::new(e2));
	}
	Ok(e)
}

fn parse_atom(p: &mut Parse) -> Result<Expr> {
	if p.pos == p.tokens.len() {
		Diag::error(p.eol, "expected expression").emit();
		return Err(Error)
	}
	macro c($e:expr => $p:pat in $o:expr) {
		if let Some($p) = $e {
			return Ok($o)
		}
	}
	c!(test!(p, Token::Paren(d) => d) => d in
		Parse::new_inner(&d.tokens, d.close, p.context).parse_with(|p| parse_expr0(p, 10))?);
	c!(test!(p, Token::Minus => ()) => () in
		Expr::Unop(ExprUnop::Neg, Box::new(parse_atom(p)?)));
	c!(test!(p, Token::Excl => ()) => () in
		Expr::Unop(ExprUnop::Not, Box::new(parse_atom(p)?)));
	c!(test!(p, Token::Tilde => ()) => () in
		Expr::Unop(ExprUnop::Inv, Box::new(parse_atom(p)?)));
	c!(TryVal::try_parse(p)? => v in Expr::Const(v));
	c!(TryVal::try_parse(p)? => v in Expr::Flag(v));
	c!(TryVal::try_parse(p)? => v in Expr::Var(v));
	c!(TryVal::try_parse(p)? => v in Expr::Attr(v));
	c!(TryVal::try_parse(p)? => v in Expr::CharAttr(v));
	c!(TryVal::try_parse(p)? => v in Expr::Global(v));
	c!(p.term("random")? => () in Expr::Rand);
	Ok(Expr::Insn(Box::new(parse_insn(p))))
}

macro op($p:ident; $t1:ident $($t:ident)* => $op:expr) {
	let pos = $p.pos;
	if test!($p, Token::$t1) $( && $p.space().is_none() && test!($p, Token::$t))* {
		return Some($op)
	}
	$p.pos = pos;
}

fn parse_assop(p: &mut Parse) -> Option<ExprUnop> {
	op!(p;         Eq => ExprUnop::Ass);
	op!(p; Plus    Eq => ExprUnop::AddAss);
	op!(p; Minus   Eq => ExprUnop::SubAss);
	op!(p; Star    Eq => ExprUnop::MulAss);
	op!(p; Slash   Eq => ExprUnop::DivAss);
	op!(p; Percent Eq => ExprUnop::ModAss);
	op!(p; Pipe    Eq => ExprUnop::OrAss);
	op!(p; Amp     Eq => ExprUnop::AndAss);
	op!(p; Caret   Eq => ExprUnop::XorAss);

	None
}

fn parse_binop(p: &mut Parse, prec: usize) -> Option<ExprBinop> {
	macro prio($prio:literal, $p:stmt) {
		if prec >= $prio {
			$p
		}
	}
	prio!(4, op!(p; Eq Eq   => ExprBinop::Eq));
	prio!(4, op!(p; Excl Eq => ExprBinop::Ne));
	prio!(4, op!(p; Lt Eq   => ExprBinop::Le));
	prio!(4, op!(p; Lt      => ExprBinop::Lt));
	prio!(4, op!(p; Gt Eq   => ExprBinop::Ge));
	prio!(4, op!(p; Gt      => ExprBinop::Gt));

	prio!(1, op!(p; Pipe Pipe => ExprBinop::Or));
	prio!(3, op!(p; Amp  Amp  => ExprBinop::BoolAnd));

	prio!(5, op!(p; Plus    => ExprBinop::Add));
	prio!(5, op!(p; Minus   => ExprBinop::Sub));
	prio!(6, op!(p; Star    => ExprBinop::Mul));
	prio!(6, op!(p; Slash   => ExprBinop::Div));
	prio!(6, op!(p; Percent => ExprBinop::Mod));
	prio!(1, op!(p; Pipe    => ExprBinop::Or));
	prio!(3, op!(p; Amp     => ExprBinop::And));
	prio!(2, op!(p; Caret   => ExprBinop::Xor));

	None
}
