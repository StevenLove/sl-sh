use quote::quote;
use quote::ToTokens;
use std::ops::Deref;
use syn::__private::{Span, TokenStream2};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::{
    parse_macro_input, AngleBracketedGenericArguments, FnArg, GenericArgument, Ident, Lit, Meta,
    NestedMeta, Path, PathArguments, PathSegment, ReturnType, Type, TypePath,
};
extern crate static_assertions;

fn get_input_types(inputs: &Punctuated<FnArg, Comma>) -> Vec<Type> {
    let mut types = vec![];
    for input in inputs {
        match input {
            FnArg::Receiver(_) => {
                unimplemented!("FnArg::Receiver is not yet implemented.")
            }
            FnArg::Typed(ty) => {
                types.push(ty.ty.deref().clone());
            }
        }
    }
    types
}

fn build_sl_sh_expression_type() -> Type {
    let crate_path_segment = PathSegment {
        ident: Ident::new("crate", Span::call_site()),
        arguments: PathArguments::None,
    };
    let exp_enum_path_segment = PathSegment {
        ident: Ident::new("Expression", Span::call_site()),
        arguments: PathArguments::None,
    };
    let mut pun_seq = Punctuated::new();
    pun_seq.push(crate_path_segment);
    pun_seq.push(exp_enum_path_segment);
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: None,
            segments: pun_seq,
        },
    })
}

/// return the function names the macro will create. Given a base name, "base"
/// return (base, builtin_base, arg_parse_base) tuple of Idents
fn get_fn_names(item_fn: &syn::ItemFn) -> (Ident, Ident, Ident, Ident) {
    let sig_ident = &item_fn.sig.ident;
    let name = sig_ident.to_string();
    let original_fn_name = Ident::new(&name, Span::call_site());
    let builtin_name = "builtin_".to_string() + &name;
    let builtin_name = Ident::new(&builtin_name, Span::call_site());
    let parse_name = "parse_".to_string() + &name;
    let parse_name = Ident::new(&parse_name, Span::call_site());
    let intern_name = "intern_".to_string() + &name;
    let intern_name = Ident::new(&intern_name, Span::call_site());
    (original_fn_name, builtin_name, parse_name, intern_name)
}

/// given the length of the rust native args list, create two lists of argument names
/// and crate::ExpEnum types for generating the builtin function signature.
fn generate_builtin_arg_list(len: usize) -> (Vec<Ident>, Vec<Type>) {
    let mut fn_args = vec![];
    let mut fn_types = vec![];
    for i in 0..len {
        let arg_name = "arg_".to_string() + &i.to_string();
        let arg = Ident::new(&arg_name, Span::call_site());
        fn_args.push(arg);
        let ty = build_sl_sh_expression_type();
        fn_types.push(ty);
    }
    (fn_args, fn_types)
}

/// given a type and the string value of a trait in std::convert::<convert_trait>
/// returned the given type wrapped with the std::convert::<convert_trait>
fn wrap_with_std_convert(ty: Type, convert_trait: &str) -> Type {
    let std_path_segment = PathSegment {
        ident: Ident::new("std", Span::call_site()),
        arguments: PathArguments::None,
    };
    let convert_path_segment = PathSegment {
        ident: Ident::new("convert", Span::call_site()),
        arguments: PathArguments::None,
    };
    let generic_argument = GenericArgument::Type(ty);
    let mut generic_pun_seq = Punctuated::new();
    generic_pun_seq.push(generic_argument);
    let generic_argument = AngleBracketedGenericArguments {
        colon2_token: None,
        lt_token: Default::default(),
        args: generic_pun_seq,
        gt_token: Default::default(),
    };
    let trait_path_segment = PathSegment {
        ident: Ident::new(convert_trait, Span::call_site()),
        arguments: PathArguments::AngleBracketed(generic_argument),
    };
    let mut pun_seq = Punctuated::new();
    pun_seq.push(std_path_segment);
    pun_seq.push(convert_path_segment);
    pun_seq.push(trait_path_segment);
    Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: None,
            segments: pun_seq,
        },
    })
}

fn get_return_type(item_fn: &syn::ItemFn) -> Type {
    let return_type = match &item_fn.sig.output {
        ReturnType::Default => {
            unimplemented!("Functions with attribute must return a value.");
        }
        ReturnType::Type(_ra_arrow, ty) => *ty.clone(),
    };
    return_type
}

// TODO fix me,
//  -   this should throw an error if no doc is found, that's required!
//  -   spacing not preserved? really??
fn get_documentation_for_fn(item_fn: &syn::ItemFn) -> String {
    for attr in &item_fn.attrs {
        for path_segment in attr.path.segments.iter() {
            if &path_segment.ident.to_string() == "doc" {
                match attr.parse_meta() {
                    Ok(meta) => match meta {
                        Meta::Path(_) => {}
                        Meta::List(_) => {}
                        Meta::NameValue(pair) => {
                            let path = &pair.path;
                            let lit = &pair.lit;
                            match (path.get_ident(), lit) {
                                (_, Lit::Str(partial_name)) => {
                                    return partial_name.value();
                                }
                                (_, _) => {
                                    unimplemented!(
                                        "0 Only support attributes of form (name = \"value\")"
                                    );
                                }
                            }
                        }
                    },
                    Err(_) => {}
                }
            }
        }
    }
    unimplemented!("Functions with attribute must have a doc comment")
}

fn generate_assertions_code_for_type_conversions(item_fn: &syn::ItemFn) -> Vec<TokenStream2> {
    let inputs = &item_fn.sig.inputs;
    let input_types = get_input_types(inputs);
    let mut conversion_assertions_code = vec![];
    for input_type in input_types {
        let try_into = wrap_with_std_convert(input_type, "TryInto");
        let exp_enum = build_sl_sh_expression_type();
        conversion_assertions_code.push(quote! {
          static_assertions::assert_impl_all!(#exp_enum: #try_into);
        });
    }
    let return_type = get_return_type(item_fn);
    let to_return_type = wrap_with_std_convert(build_sl_sh_expression_type(), "Into");
    conversion_assertions_code.push(quote! {
      static_assertions::assert_impl_all!(#return_type: #to_return_type);
    });
    conversion_assertions_code
}

fn get_attribute_value_with_key(key: &str, values: &[(String, String)]) -> Option<String> {
    let pair = values.iter().filter(|k| k.0 == key).take(1).next();
    pair.map(|pair| pair.1.to_string())
}

fn get_attribute_name_pair(nested_meta: &NestedMeta) -> Option<(String, String)> {
    match nested_meta {
        NestedMeta::Meta(meta) => match meta {
            Meta::NameValue(pair) => {
                let path = &pair.path;
                let lit = &pair.lit;
                match (path.get_ident(), lit) {
                    (Some(ident), Lit::Str(partial_name)) => {
                        Some((ident.to_string(), partial_name.value()))
                    }
                    (_, _) => {
                        unimplemented!("0 Only support attributes of form (name = \"value\")");
                    }
                }
            }
            _ => {
                unimplemented!("1 Only support attributes of form (name = \"value\")");
            }
        },
        NestedMeta::Lit(_) => {
            unimplemented!("2 Only support attributes of form (name = \"value\")");
        }
    }
}

fn generate_sl_sh_fns(item_fn: &syn::ItemFn, attr_args: syn::AttributeArgs) -> TokenStream2 {
    let vals = attr_args
        .iter()
        .filter_map(get_attribute_name_pair)
        .collect::<Vec<(String, String)>>();
    let fn_name_attr = "fn_name".to_string();
    let fn_name = get_attribute_value_with_key(&fn_name_attr, &vals)
        .expect("Attribute 'fn_name' name-value pair must be set.");
    let fn_name_attr = Ident::new(&fn_name_attr, Span::call_site());

    let doc_comments = get_documentation_for_fn(item_fn);
    let conversions_assertions_code = generate_assertions_code_for_type_conversions(item_fn);

    let args_len = item_fn.sig.inputs.len();
    let (fn_args, fn_types) = generate_builtin_arg_list(args_len);
    let (original_fn_name, builtin_name, parse_name, intern_name) = get_fn_names(item_fn);

    quote! {
        fn #builtin_name(#(#fn_args: #fn_types),*) -> crate::LispResult<crate::types::Expression> {
            use std::convert::TryInto;
            use std::convert::Into;
            use crate::builtins_util::TryIntoExpression;
            let #fn_name_attr = #fn_name;
            #(#conversions_assertions_code)*
            let result = #original_fn_name(#(#fn_args.try_into_for(#fn_name_attr)?),*);
            Ok(result.into())
        }

        fn #parse_name(
            environment: &mut crate::environment::Environment,
            args: &mut dyn Iterator<Item = crate::types::Expression>,
        ) -> crate::LispResult<crate::types::Expression> {
            use std::convert::TryInto;
            use crate::builtins_util::ExpandVecToArgs;
            let args = crate::builtins_util::make_args(environment, args)?;
            let #fn_name_attr = #fn_name;
            const args_len: usize = #args_len;
            if args.len() == args_len {
                let params: [crate::types::Expression; args_len] = args.try_into().expect("sl_sh_fn proc_macro_attribute has incorrect information about arity of function it decorates.");
                #builtin_name.call_expand_args(params)
            } else if args.len() > args_len {
                Err(LispError::new(format!("{} given too many arguments, expected {}, got {}.", #fn_name_attr, args_len, args.len())))
            } else {
                Err(LispError::new(format!("{} not given enough arguments, expected {}, got {}.", #fn_name_attr, args_len, args.len())))
            }
        }

        fn #intern_name<S: std::hash::BuildHasher>(
            interner: &mut Interner,
            data: &mut std::collections::HashMap<&'static str, (crate::types::Expression, String), S>,
        ) {
            let #fn_name_attr = #fn_name;
            data.insert(
                interner.intern(#fn_name_attr),
                crate::types::Expression::make_function(#parse_name, #doc_comments),
            );
        }
    }
}

#[proc_macro_attribute]
pub fn sl_sh_fn(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attr_args = parse_macro_input!(attr as syn::AttributeArgs);

    let tokens = match syn::parse::<syn::Item>(input) {
        Ok(item) => match &item {
            syn::Item::Fn(item_fn) => {
                let generated_sl_sh_fns: TokenStream2 =
                    generate_sl_sh_fns(item_fn, attr_args).into();
                let original_fn_code = item.into_token_stream();
                quote! {
                    #original_fn_code

                    #generated_sl_sh_fns
                }
            }
            _ => syn::Error::new(item.span(), "This attribute only supports functions.")
                .to_compile_error()
                .into(),
        },
        Err(e) => syn::Error::new(e.span(), "Failed to parse proc_macro_attr.")
            .to_compile_error()
            .into(),
    };

    proc_macro::TokenStream::from(tokens)
}

//TODO
//  - functions that do not return anything
//  - functions that take actual Expressions, s.t. doing into on them might... be redundant?
//  - support Option-al argument
//  - variadic functions
