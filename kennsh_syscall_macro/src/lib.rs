mod no_wrap;
mod match_error;

extern crate proc_macro;
use quote::{ToTokens, quote, quote_spanned};
use syn::{Arm, Expr, ExprPath, Ident, braced, parenthesized, parse::{Parse, ParseStream, Parser}, punctuated::Punctuated};

struct Syscall();

impl Syscall {
	fn parse(stream: ParseStream<'_>) -> syn::Result<proc_macro2::TokenStream> {
		let syscall_fn: ExprPath = stream.parse()?;
		let mut syscall_params = None;
		if stream.peek(syn::token::Paren) {
			let tmp;
			parenthesized!(tmp in stream);
			syscall_params = tmp.parse_terminated::<Expr, syn::Token![,]>(Expr::parse).ok();
		}
		let syscall_params = syscall_params.unwrap_or_else(|| {
			Punctuated::new()
		});
		
		// Either ; or match
		if stream.is_empty() {
			Ok(gen_arrow(
				syscall_fn,
				syscall_params.into_iter(),
				quote!{ __ok },
				quote!{ Ok(__ok) },
				([] as [&str; 0]).iter(),
			))
		}
		else if stream.peek(syn::Token![;]) {
			Self::parse_arrow_2(
				&stream,
				syscall_fn, 
				syscall_params.into_iter()
			)
		}
		else if stream.peek(syn::Token![match]) {
			Self::parse_match_2(
				&stream,
				syscall_fn, 
				syscall_params.into_iter()
			)
		}
		else {
			Ok(quote_spanned! {
				stream.span()=>
				compile_error!("Expected either just the syscall invocation, a semicolor or a match keywork");
			})
		}
	}

	fn parse_arrow_2<TT1, TT2Item, TT2>(
		stream: &ParseStream<'_>,
		syscall_fn: TT1, 
		syscall_params: TT2) -> syn::Result<proc_macro2::TokenStream>
		where TT1: ToTokens,
			TT2Item: ToTokens,
			TT2: IntoIterator<Item= TT2Item> {
		stream.parse::<syn::Token!(;)>()?;
		let ok_expression = if !stream.peek(crate::match_error::match_error) {
			let (ok_name, nowrap) = if stream.peek2(syn::Token![=>]) {
				let ok_name: Ident = stream.parse()?;
				stream.parse::<syn::Token!(=>)>()?;
				let nowrap = match stream.parse::<crate::no_wrap::no_wrap>() {
					Ok(_) => true,
					Err(_) => false
				};
				(ok_name, nowrap)
			}
			else {
				let nowrap = match stream.parse::<crate::no_wrap::no_wrap>() {
					Ok(_) => true,
					Err(_) => false
				};
				(Ident::new("it", proc_macro2::Span::call_site()), nowrap)
			};

			
			let expr: Expr = stream.parse()?;
			(
				quote!( #ok_name ),
				if nowrap {
					quote!( #expr )
				}
				else {
					quote!( Ok(#expr) )
				}
			)
		}
		else {
			(quote!(__ok), quote!(Ok(__ok)))
		};
		let custom_error_arms = if let Ok(_) = stream.parse::<crate::match_error::match_error>() {
			let tmp;
			braced!(tmp in stream);
			let mut arms: Vec<Arm> = vec![];
			while !tmp.is_empty() {
				let parsed = tmp.parse()?;
				arms.push(parsed);
			}
			arms
		} else {
			vec![]
		};
		
		Ok(gen_arrow(
			syscall_fn, 
			syscall_params.into_iter(), 
			ok_expression.0, 
			ok_expression.1, 
			custom_error_arms.into_iter(),
		))
	}

	fn parse_match_2<TT1, TT2Item, TT2>(
		stream: &ParseStream<'_>,
		syscall_fn: TT1, 
		syscall_params: TT2) -> syn::Result<proc_macro2::TokenStream>
		where TT1: ToTokens,
			TT2Item: ToTokens,
			TT2: IntoIterator<Item= TT2Item> {
		stream.parse::<syn::Token![match]>()?;
		let nowrap = match stream.parse::<crate::no_wrap::no_wrap>() {
			Ok(_) => true,
			Err(_) => false
		};
		let match_block_contents = {
			let tmp;
			braced!(tmp in stream);
			let mut arms = Vec::<Arm>::new();
			while !tmp.is_empty() {
				let parsed = tmp.parse()?;
				arms.push(parsed);
			}
			arms
		}.into_iter();
		let match_block_contents = match_block_contents.map(|b| {
			if b.comma.is_some() {
				quote! { #b }
			}
			else {
				quote! { #b, }
			}
		});
		let ok_name = quote! { __ok };
		let expr = quote! {
			match #ok_name {
				#( #match_block_contents )*
			}
		};
		let custom_error_arms = if let Ok(_) = stream.parse::<crate::match_error::match_error>() {
			let tmp;
			braced!(tmp in stream);
			let mut arms: Vec<Arm> = vec![];
			while !tmp.is_empty() {
				let parsed = tmp.parse()?;
				arms.push(parsed);
			}
			arms
		} else {
		    vec![]
		};

		Ok(
			gen_arrow(
				syscall_fn,
				syscall_params.into_iter(),
				ok_name.clone(),
				if nowrap {
					expr
				} else {
					quote! {
						Ok(#expr)
					}
				},
				custom_error_arms.into_iter(),
			)
		)
	}
}

fn gen_arrow<TT1, TmpTT1, TT2, TT3, TT4, TmpTT2, TT5>(
	syscall_fn: TT1, 
	syscall_params: TT2, 
	ok_name: TT3, 
	expression: TT4,
	custom_error_arms: TT5) -> proc_macro2::TokenStream 
	where TT1: ToTokens,
		TmpTT1: ToTokens,
		TT2: Iterator<Item=TmpTT1>,
		TT3: ToTokens,
		TT4: ToTokens,
		TmpTT2: ToTokens,
		TT5: Iterator<Item=TmpTT2> {
	quote! {
		match #syscall_fn (
			#( #syscall_params ),*
		) {
			Ok(#ok_name) => #expression,
			Err(err) => Err(match err { 
				#( #custom_error_arms )*
				err => crate::error::Error::SyscallError {
					call_name: stringify!(#syscall_fn).to_string(),
					error: err,
				}
			})
		}
	}
}

#[proc_macro]
pub fn syscall(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
	// match None.ok_or(
	// 	syn::Error::new(Span::call_site(), "")
	// ).or_else(|_| {
	// 	Syscall::parse_arrow.parse(item.clone())
	// }).or_else( |_| {
	// 	Syscall::parse_match.parse(item.clone())
	// }) {
	match Syscall::parse.parse(item.clone()) {
		Ok(data) => data.into(),
		Err(err) => proc_macro::TokenStream::from(err.to_compile_error())
	}
}
