#![feature(proc_macro_diagnostic)]
#![feature(let_chains)]

use std::collections::BTreeMap;

use convert_case::{Case, Casing, Boundary};
use proc_macro::{TokenStream as TokenStream0, Diagnostic, Level};
use proc_macro2::{TokenStream, Span};
use quote::{quote, format_ident, ToTokens};
use syn::{
	*,
	spanned::Spanned,
	punctuated::*,
};

mod parse;
use parse::*;

macro_rules! q {
	(_      => $($b:tt)*) => { ::quote::quote!         {                $($b)* } };
	($a:expr=> $($b:tt)*) => { ::quote::quote_spanned! { ($a).span() => $($b)* } };
}

macro_rules! pq {
	(_      => $($b:tt)*) => { ::syn::parse_quote!         {                $($b)* } };
	($a:expr=> $($b:tt)*) => { ::syn::parse_quote_spanned! { ($a).span() => $($b)* } };
}

// {{{1 Main
#[proc_macro]
#[allow(non_snake_case)]
pub fn bytecode(tokens: TokenStream0) -> TokenStream0 {
	let input: Top = parse_macro_input!(tokens);
	let ctx = match gather_top(input) {
		Ok(ctx) => ctx,
		Err(err) => return err.into_compile_error().into()
	};

	let func_args = &ctx.func_args;
	let attrs = &ctx.attrs;
	let game_expr = &ctx.game_expr;
	let game_ty = &ctx.game_ty;

	let read = ctx.reads.iter().map(|ReadArm { span, games, body }| {
		let games_name = games.iter().map(|a| &a.0).collect::<Vec<_>>();
		let games_hex  = games.iter().map(|a| &a.1).collect::<Vec<_>>();
		q!{span=>
			#((IS::#games_name, #games_hex))|* => {
				run(__f, #body)
			}
		}
	}).collect::<TokenStream>();
	let read = quote! {
		pub fn read<'a>(__f: &mut impl In<'a>, #func_args) -> Result<Self, ReadError> {
			fn run<'a, I: In<'a>, A>(__f: &mut I, fun: impl FnOnce(&mut I) -> Result<A, ReadError>) -> Result<A, ReadError> {
				fun(__f)
			}
			type IS = #game_ty;
			match (#game_expr, __f.u8()?) {
				#read
				(_g, _v) => Err(format!("invalid Insn on {:?}: 0x{:02X}", _g, _v).into())
			}
		}
	};

	let write = ctx.writes.iter().map(|WriteArm { span, games, ident, args, body }| {
		let games_name = games.iter().map(|a| &a.0).collect::<Vec<_>>();
		let games_hex  = games.iter().map(|a| &a.1).collect::<Vec<_>>();
		q!{span=>
			(__iset@(#(IS::#games_name)|*), Self::#ident(#args)) => {
				__f.u8(match __iset {
					#(IS::#games_name => #games_hex,)*
					_g => unsafe { std::hint::unreachable_unchecked() }
				});
				run(__f, #body)?;
			}
		}
	}).collect::<TokenStream>();
	let write = quote! {
		pub fn write(__f: &mut impl OutDelay, #func_args, __insn: &Insn) -> Result<(), WriteError> {
			fn run<O: OutDelay>(__f: &mut O, fun: impl FnOnce(&mut O) -> Result<(), WriteError>) -> Result<(), WriteError> {
				fun(__f)
			}
			type IS = #game_ty;
			#[allow(unused_parens)]
			match (#game_expr, __insn) {
				#write
				(_is, _i) => return Err(format!("`{}` is not supported on `{:?}`", _i.name(), _is).into())
			}
			Ok(())
		}
	};

	let doc_insn_table = make_table(&ctx);

	let Insn_body = ctx.defs.iter().map(|Insn { span, attrs, ident, types, aliases, .. }| {
		let mut predoc = String::new();
		predoc.push_str("**`");
		predoc.push_str(&ident.to_string());
		predoc.push_str("`**");
		for alias in aliases {
			predoc.push_str(&format!(" [`{alias}`](InsnArg::{alias})"));
		}
		predoc.push_str("\n\n");

		q!{span=>
			#[doc = #predoc]
			#attrs
			#ident(#(#types),*),
		}
	}).collect::<TokenStream>();

	let main = quote! {
		#[allow(non_camel_case_types)]
		#[derive(Debug, Clone, PartialEq, Eq)]
		#attrs
		/// # Encoding
		/// Below is a table listing the hex codes for each instruction.
		/// This can for example be used to see which instructions are available in each game.
		/// Though do keep in mind that this is only based on research; it may not fully reflect what the games actually accept.
		// /// <details><summary>Click to expand</summary>
		#[doc = #doc_insn_table]
		// /// </details>
		pub enum Insn {
			#Insn_body
		}

		impl Insn {
			#read
			#write
		}
	};

	let InsnArg_names = ctx.arg_types.keys().collect::<Vec<_>>();
	let InsnArg_types = ctx.arg_types.values().collect::<Vec<_>>();

	let name_body = ctx.defs.iter().map(|Insn { span, ident, .. }| {
		q!{span=>
			Self::#ident(..) => stringify!(#ident),
		}
	}).collect::<TokenStream>();

	let args_body = ctx.defs.iter().map(|Insn { span, ident, args, aliases, .. }| {
		q!{span=>
			Self::#ident(#(#args),*) => Box::new([#(Arg::#aliases(#args)),*]),
		}
	}).collect::<TokenStream>();

	let arg_types_body = ctx.defs.iter().map(|Insn { span, ident, aliases, .. }| {
		q!{span=>
			stringify!(#ident) => Box::new([#(Arg::#aliases),*]),
		}
	}).collect::<TokenStream>();

	let from_args_body = ctx.defs.iter().map(|Insn { span, ident, args, aliases, .. }| {
		q!{span=>
			stringify!(#ident) => {
				#(let #args = if let Some(Arg::#aliases(v)) = it.next() { v } else { return None; };)*
				Self::#ident(#(#args),*)
			},
		}
	}).collect::<TokenStream>();

	let introspection = quote! {
		#[cfg(not(doc))]
		#[allow(non_camel_case_types)]
		#[derive(Debug, Clone)]
		pub enum InsnArgOwned {
			#(#InsnArg_names(#InsnArg_types),)*
		}

		#[cfg(not(doc))]
		#[allow(non_camel_case_types)]
		#[derive(Debug, Clone, Copy)]
		pub enum InsnArg<'a> {
			#(#InsnArg_names(&'a #InsnArg_types),)*
		}

		#[cfg(not(doc))]
		#[allow(non_camel_case_types)]
		#[derive(Debug)]
		pub enum InsnArgMut<'a> {
			#(#InsnArg_names(&'a mut #InsnArg_types),)*
		}

		#[cfg(not(doc))]
		#[allow(non_camel_case_types)]
		#[derive(Debug, Clone, Copy)]
		pub enum InsnArgType {
			#(#InsnArg_names,)*
		}

		// doc shims
		#[cfg(doc)]
		#[doc(alias="InsnArgOwned")]
		#[doc(alias="InsnArgMut")]
		#[doc(alias="InsnArgType")]
		#[allow(non_camel_case_types)]
		#[derive(Debug)]
		pub enum InsnArg {
			#(#InsnArg_names(#InsnArg_types),)*
		}

		#[cfg(doc)]
		#[doc(inline, hidden)]
		pub use InsnArg as InsnArgOwned;

		#[cfg(doc)]
		#[doc(inline, hidden)]
		pub use InsnArg as InsnArgMut;

		#[cfg(doc)]
		#[doc(inline, hidden)]
		pub use InsnArg as InsnArgType;

		impl Insn {
			pub fn name(&self) -> &'static str {
				match self {
					#name_body
				}
			}

			pub fn args(&self) -> Box<[InsnArg]> {
				use InsnArg as Arg;
				match self {
					#args_body
				}
			}

			pub fn args_mut(&mut self) -> Box<[InsnArgMut]> {
				use InsnArgMut as Arg;
				match self {
					#args_body
				}
			}

			pub fn into_parts(self) -> (&'static str, Box<[InsnArgOwned]>) {
				use InsnArgOwned as Arg;
				let name = self.name();
				let args: Box<[Arg]> = match self {
					#args_body
				};
				(name, args)
			}

			pub fn arg_types(name: &str) -> Option<Box<[InsnArgType]>> {
				use InsnArgType as Arg;
				let types: Box<[Arg]> = match name {
					#arg_types_body
					_ => return None,
				};
				Some(types)
			}

			pub fn from_parts(name: &str, args: impl IntoIterator<Item=InsnArgOwned>) -> Option<Insn> {
				use InsnArgOwned as Arg;
				let mut it = args.into_iter();
				let v = match name {
					#from_args_body
					_ => return None,
				};
				if let Some(_) = it.next() { return None; }
				Some(v)
			}
		}
	};

	quote! {
		#main
		#introspection
	}.into()
}

struct Ctx {
	arg_types: BTreeMap<Ident, Box<Type>>,
	func_args: Punctuated<PatType, Token![,]>,
	games: Vec<Ident>,
	attrs: Attributes,
	defs: Vec<Insn>,
	reads: Vec<ReadArm>,
	writes: Vec<WriteArm>,
	game_expr: Box<Expr>,
	game_ty: Box<Type>,
}

#[derive(Clone)]
struct InwardContext {
	ident: Ident,
	attrs: Attributes,
	args: Punctuated<Ident, Token![,]>,
	aliases: Vec<Ident>,
	types: Vec<Box<Type>>,
	games: GameSpec,
	write: TokenStream,
}

type GameSpec = Vec<(Ident, u8)>;

struct Insn {
	span: Span,
	ident: Ident,
	attrs: Attributes,
	args: Vec<Ident>,
	aliases: Vec<Ident>,
	types: Vec<Box<Type>>,
}

struct ReadArm {
	span: Span,
	games: GameSpec,
	body: Box<Expr>,
}

struct WriteArm {
	span: Span,
	games: GameSpec,
	ident: Ident,
	args: Punctuated<Ident, Token![,]>,
	body: Box<Expr>,
}

fn make_table(ctx: &Ctx) -> String {
	let doc = choubun::node("table", |n| {
		let mut hex: BTreeMap<Ident, BTreeMap<Ident, u8>> = BTreeMap::new();
		for WriteArm { games, ident, .. } in &ctx.writes {
			let entry = hex.entry(ident.clone()).or_default();
			for (game, hex) in games {
				entry.insert(game.clone(), *hex);
			}
		}

		n.attr("style", "text-align: center; width: min-content; overflow-x: unset");
		n.node("thead", |n| {
			n.attr("style", "position: sticky; top: 0");
			n.node("tr", |n| {
				n.node("th", |_| {});
				for game in &ctx.games {
					n.node("th", |n| {
						n.attr("style", "writing-mode: vertical-lr");
						let ty = &ctx.game_ty;
						n.text(format_args!("[`{game}`]({ty}::{game})", ty=q!{_=> #ty }))
					});
				}
			});
		});

		n.node("tbody", |n| {
			let mut insns = ctx.defs.iter().peekable();
			while let Some(def) = insns.next() {
				let games = hex.get(&def.ident).unwrap();
				let mut defs = vec![def];
				while let Some(next) = insns.peek() && hex.get(&next.ident).unwrap() == games {
					defs.push(insns.next().unwrap());
				}

				n.node("tr", |n| {
					n.node("td", |n| {
						n.attr("style", "text-align: left");
						for Insn { ident, aliases, ..} in defs {
							let mut title = String::new();
							title.push_str(&ident.to_string());
							for alias in aliases {
								title.push_str(&format!(" {alias}"));
							}
							n.node("span", |n| {
								n.attr("title", title);
								n.text(format_args!("[`{ident}`](Self::{ident})"));
							});
							n.text(" ");
						}
					});

					let mut columns = Vec::<choubun::Node>::new();
					let mut prev = None;
					for game in &ctx.games {
						let node = choubun::node("td", |n| {
							n.attr("style", "vertical-align: middle");
							let hex = games.get(game);
							if let Some(hex) = hex {
								n.text(format_args!("{hex:02X}"));
							}
							if prev == Some(hex) {
								let prev = columns.last_mut().unwrap();
								n.attrs_mut().get_mut("style").unwrap().push_str("; border-left: none");
								prev.attrs_mut().get_mut("style").unwrap().push_str("; border-right: none");
							}
							prev = Some(hex);
						});
						columns.push(node);
					}
					for item in columns {
						n.add(item);
					}
				});
			}
		});
	});
	let doc = choubun::node("div", |n| {
		n.class("example-wrap");
		n.add(doc)
	});
	format!("\n\n<span></span>{}\n\n", doc.render_to_string())
}

fn gather_top(input: Top) -> Result<Ctx> {
	let games = input.attrs.games;
	let all_games: Box<[Ident]> = games.idents.iter().cloned().collect();

	let mut ctx = Ctx {
		arg_types: BTreeMap::new(),
		func_args: input.args,
		attrs: input.attrs.other,
		games: games.idents.iter().cloned().collect(),
		defs: Vec::new(),
		reads: Vec::new(),
		writes: Vec::new(),
		game_expr: games.expr,
		game_ty: games.ty,
	};

	// Used in the dump
	ctx.arg_types.insert(Ident::new("String", Span::call_site()), parse_quote! { String });

	let mut n = vec![0; games.idents.len()];
	for item in input.defs {
		match item {
			Def::Skip(mut def) => {
				let val = def.count.base10_parse::<u8>()?;

				get_games(&mut def.attrs, &all_games, &mut n, val as usize)?;

				for attr in def.attrs.other.iter() {
					Diagnostic::spanned(attr.path.span().unwrap(), Level::Error, format!("cannot find attribute `{}` in this scope", attr.path.to_token_stream())).emit();
				}
			}
			Def::Custom(mut def) => {
				let games = get_games(&mut def.attrs, &all_games, &mut n, 1)?;

				for attr in def.attrs.other.iter() {
					Diagnostic::spanned(attr.path.span().unwrap(), Level::Error, format!("cannot find attribute `{}` in this scope", attr.path.to_token_stream())).emit();
				}

				let mut has_read = false;
				for clause in def.clauses {
					match clause {
						Clause::Read(clause) => {
							if has_read {
								Diagnostic::spanned(clause.read_token.span().unwrap(), Level::Error, "only one `read` allowed").emit();
							}
							has_read = true;

							ctx.reads.push(ReadArm {
								span: clause.span(),
								games: games.clone(),
								body: clause.expr,
							});
						}
						Clause::Write(clause) => {
							ctx.writes.push(WriteArm {
								span: clause.span(),
								games: games.clone(),
								ident: clause.ident,
								args: clause.args,
								body: clause.expr,
							});
						}
					}
				}
			}
			Def::Standard(mut def) => {
				let span = def.span();
				let games = get_games(&mut def.attrs, &all_games, &mut n, 1)?;

				let ictx = InwardContext {
					ident: def.ident.clone(),
					attrs: def.attrs.other.clone(),
					args: Punctuated::new(),
					aliases: Vec::new(),
					types: Vec::new(),
					games: games.clone(),
					write: TokenStream::new(),
				};
				let read = gather_arm(&mut ctx, ictx, def);
				ctx.reads.push(ReadArm {
					span,
					games: games.clone(),
					body: pq!{read=> |__f| { #read } },
				});
			}
		}
	}

	for (game, n) in all_games.iter().zip(n.iter()) {
		if *n != 256 {
			Diagnostic::spanned(input.end_span.unwrap(), Level::Warning, format!("instructions do not sum up to 256: {game} => {n}")).emit();
		}
	}

	Ok(ctx)
}

fn get_games(attrs: &mut DefAttributes, all_games: &[Ident], n: &mut [usize], num: usize) -> Result<GameSpec> {
	let games = if let Some(attr) = &attrs.game {
		attr.idents.iter().cloned().collect()
	} else {
		all_games.to_owned()
	};

	let game_idx: Vec<usize> = games.iter().filter_map(|game| {
		if let Some(n) = all_games.iter().position(|a| a == game) {
			Some(n)
		} else {
			Diagnostic::spanned(game.span().unwrap(), Level::Error, format!("unknown game `{game}`")).emit();
			None
		}
	}).collect();

	let games = games.iter().cloned()
		.zip(game_idx.iter().map(|idx| n[*idx] as u8))
		.collect::<GameSpec>();

	for idx in &game_idx {
		n[*idx] += num;
	}

	Ok(games)
}

fn gather_arm(ctx: &mut Ctx, mut ictx: InwardContext, arm: DefStandard) -> TokenStream {
	let mut read = TokenStream::new();
	let span = arm.span();

	for pair in arm.args.into_pairs() {
		match pair.into_tuple() {
			(Arg::Standard(arg), _) => {
				let varname = format_ident!("_{}", ictx.args.len(), span=arg.span());

				let mut types = Vec::new();
				types.push(match &arg.source {
					Source::Simple(name) => {
						q!{name=> #name }
					}
					Source::Call(a) => {
						let ty = &a.ty;
						q!{ty=> #ty }
					}
				});
				for ty in &arg.ty {
					let ty_ = &ty.ty;
					types.push(q!{ty=> #ty_});
				}

				{
					let mut val = match &arg.source {
						Source::Simple(name) => {
							let name = to_snake(name);
							q!{name=> __f.#name()? }
						},
						Source::Call(a) => {
							let name = &a.name;
							let mut args = vec![q!{a=> __f }];
							for e in &a.args {
								args.push(
									if let Expr::Path(ExprPath { attrs, qself: None, path }) = &**e
										&& attrs.is_empty()
										&& let Some(ident) = path.get_ident()
									{
										if ictx.args.iter().any(|a| a == ident) {
											q!{ident=> &#ident }
										} else {
											q!{ident=> #ident }
										}
									} else {
										let v = ictx.args.iter();
										q!{e=> #[allow(clippy::let_and_return)] { #(let #v = &#v;)* #e } }
									}
								);
							}
							q!{a=> #name::read(#(#args),*)? }
						},
					};
					for ty in types.iter().skip(1) {
						val = q!{ty=> cast::<_, #ty>(#val)? };
					}
					read.extend(q!{arg=> let #varname = #val; });
				}

				{
					let mut val = q!{varname=> #varname };
					if let Source::Simple(a) = &arg.source {
						if a != "String" {
							val = q!{arg=> *#val };
						}
					}
					for ty in types.iter().rev().skip(1) {
						val = q!{ty=> cast::<_, #ty>(#val)? };
					}
					val = match &arg.source {
						Source::Simple(name) => {
							let name = to_snake(name);
							q!{name=> __f.#name(#val) }
						},
						Source::Call(a) => {
							let name = &a.name;
							let mut args = vec![q!{a=> __f }];
							for e in &a.args {
								args.push(q!{e=> #e })
							}
							args.push(val);
							q!{a=> #name::write(#(#args),*)? }
						},
					};
					if let Source::Simple(a) = &arg.source {
						if a == "String" {
							val = q!{arg=> #val? };
						}
					}
					ictx.write.extend(q!{arg=> #val; });
				}

				let ty = if let Some(ty) = &arg.ty.last() {
					ty.ty.clone()
				} else {
					match &arg.source {
						Source::Simple(ident) => pq!{ _ => #ident },
						Source::Call(s) => s.ty.clone(),
					}
				};
				let alias = 'a: {
					let ty = if let Some(alias) = &arg.alias {
						break 'a alias.ident.clone();
					} else if let Some(ty) = &arg.ty.last() {
						&ty.ty
					} else {
						match &arg.source {
							Source::Simple(ident) => break 'a ident.clone(),
							Source::Call(s) => &s.ty,
						}
					};

					if let Type::Path(ty) = Box::as_ref(ty) {
						if let Some(ident) = ty.path.get_ident() {
							break 'a ident.clone()
						}
					}

					Diagnostic::spanned(ty.span().unwrap(), Level::Error, "invalid identifier").emit();
					Ident::new("__error", Span::call_site())
				};

				ictx.args.push(varname.clone());
				ictx.aliases.push(alias.clone());
				ictx.types.push(ty.clone());
				if !ctx.arg_types.contains_key(&alias) {
					// collisions will be errored about at type checking
					ctx.arg_types.insert(alias.clone(), ty);
				}
			}
			(Arg::Split(arg), _) => {
				todo!()
			}
			(Arg::Tail(arg), comma) => {
				let mut arms = Vec::new();
				for arm in arg.arms {
					let mut ictx = ictx.clone();
					ictx.ident = format_ident!("{}{}", &ictx.ident, &arm.def.ident, span=arm.def.ident.span());
					ictx.attrs.extend((*arm.attrs).clone());
					let key = &arm.key;
					ictx.write.extend(q!{arm=> __f.u8(#key); });
					let span = arm.span();
					let body = gather_arm(ctx, ictx, arm.def);
					arms.push(q!{span=> #key => { #body } });
				}
				let name = &ictx.ident;
				read.extend(q!{span=>
					match __f.u8()? {
						#(#arms)*
						_v => Err(format!("invalid Insn::{}*: 0x{:02X}", stringify!(#name), _v).into())
					}
				});
				if let Some(comma) = comma {
					Diagnostic::spanned(comma.span().unwrap(), Level::Error, "..match {} must be last").emit();
				}
				return read
			}
		};
	}

	let ident = &ictx.ident;
	let args = &ictx.args;
	read.extend(q!{span=> Ok(Self::#ident(#args)) });

	ctx.defs.push(Insn {
		span,
		ident: ictx.ident.clone(),
		attrs: ictx.attrs,
		args: ictx.args.iter().cloned().collect(),
		aliases: ictx.aliases,
		types: ictx.types,
	});

	let write = ictx.write;
	ctx.writes.push(WriteArm {
		span,
		games: ictx.games,
		ident: ictx.ident,
		args: ictx.args,
		body: pq!{span=> |__f| { #write Ok(()) } },
	});

	read
}

fn to_snake(ident: &Ident) -> Ident {
	Ident::new(
		&ident.to_string().with_boundaries(&[Boundary::LowerUpper]).to_case(Case::Snake),
		ident.span(),
	)
}
