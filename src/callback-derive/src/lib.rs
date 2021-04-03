extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{AttributeArgs, Ident, ItemStruct, Lit, Meta, NestedMeta, Result, Type, TypePath, parse_macro_input};

#[proc_macro_attribute]
pub fn completed_callback(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr = parse_macro_input!(attr as AttributeArgs);
    let ast = parse_macro_input!(input as ItemStruct);
    impl_completed_callback(&attr, &ast).expect("error in impl_completed_callback")
}

fn impl_completed_callback(args: &AttributeArgs, ast: &ItemStruct) -> Result<TokenStream> {
    let name = &ast.ident;
    let closure = get_closure(name);

    let gen = match parse_arguments(args)? {
        (
            Some(Type::Path(ref interface)),
            Some(Type::Path(ref arg_1)),
            Some(Type::Path(ref arg_2)),
        ) => {
            let abi = get_abi(interface);

            quote! {
                type #closure = CompletedClosure<<#arg_1 as ClosureArg>::Output, <#arg_2 as ClosureArg>::Output>;

                #[repr(C)]
                pub struct #name {
                    vtable: *const #abi,
                    refcount: AtomicU32,
                    completed: Option<#closure>,
                }

                impl Callback for #name {
                    type Interface = #interface;
                    type Closure = #closure;

                    fn new(completed: #closure) -> Self {
                        static VTABLE: #abi = #abi(
                            #name::query_interface,
                            #name::add_ref,
                            #name::release,
                            #name::invoke,
                        );

                        Self {
                            vtable: &VTABLE,
                            refcount: AtomicU32::new(1),
                            completed: Some(completed),
                        }
                    }
                }

                impl CallbackInterface<#name> for #name {
                    fn refcount(&self) -> &AtomicU32 {
                        &self.refcount
                    }
                }

                impl CompletedCallback<#name, #arg_1, #arg_2> for #name {
                    fn completed(&mut self) -> Option<#closure> {
                        self.completed.take()
                    }
                }
            }
        }
        _ => panic!("expected interface + arg_1 + arg_2"),
    };

    Ok(gen.into())
}

#[proc_macro_attribute]
pub fn event_callback(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr = parse_macro_input!(attr as AttributeArgs);
    let ast = parse_macro_input!(input as ItemStruct);
    impl_event_callback(&attr, &ast).expect("error in impl_event_callback")
}

fn impl_event_callback(args: &AttributeArgs, ast: &ItemStruct) -> Result<TokenStream> {
    let name = &ast.ident;
    let closure = get_closure(name);

    let gen = match parse_arguments(args)? {
        (
            Some(Type::Path(ref interface)),
            Some(Type::Path(ref arg_1)),
            Some(Type::Path(ref arg_2)),
        ) => {
            let abi = get_abi(interface);

            quote! {
                type #closure = EventClosure<<#arg_1 as ClosureArg>::Output, <#arg_2 as ClosureArg>::Output>;

                #[repr(C)]
                pub struct #name {
                    vtable: *const #abi,
                    refcount: AtomicU32,
                    event: #closure,
                }

                impl Callback for #name {
                    type Interface = #interface;
                    type Closure = #closure;

                    fn new(event: #closure) -> Self {
                        static VTABLE: #abi = #abi(
                            #name::query_interface,
                            #name::add_ref,
                            #name::release,
                            #name::invoke,
                        );

                        Self {
                            vtable: &VTABLE,
                            refcount: AtomicU32::new(1),
                            event,
                        }
                    }
                }

                impl CallbackInterface<#name> for #name {
                    fn refcount(&self) -> &AtomicU32 {
                        &self.refcount
                    }
                }

                impl EventCallback<#name, #arg_1, #arg_2> for #name {
                    fn event(&mut self) -> &mut #closure {
                        &mut self.event
                    }
                }
            }
        }
        _ => panic!("expected interface + arg_1 + arg_2"),
    };

    Ok(gen.into())
}

fn parse_arguments(args: &AttributeArgs) -> Result<(Option<Type>, Option<Type>, Option<Type>)> {
    let mut interface = None;
    let mut arg_1 = None;
    let mut arg_2 = None;

    for arg in args {
        match arg {
            NestedMeta::Meta(Meta::NameValue(name_value)) => {
                let ident = match name_value.path.get_ident() {
                    Some(ident) => ident,
                    None => {
                        return Err(syn::Error::new(Span::call_site(), "expected an identifier"));
                    }
                };

                match (ident.to_string().as_str(), &name_value.lit) {
                    ("interface", Lit::Str(value)) => {
                        interface = Some(value.parse::<Type>()?);
                    }
                    ("arg_1", Lit::Str(value)) => {
                        arg_1 = Some(value.parse::<Type>().expect("arg_1: value.parse::<Type>()"));
                    }
                    ("arg_2", Lit::Str(value)) => {
                        arg_2 = Some(value.parse::<Type>().expect("arg_2: value.parse::<Type>()"));
                    }
                    _ => panic!("expected interface | arg_1 | arg_2"),
                };
            }
            _ => panic!("expected name/value pairs"),
        }
    }

    Ok((interface, arg_1, arg_2))
}

fn get_closure(name: &Ident) -> Ident {
    format_ident!("{}Closure", name)
}

fn get_abi(interface: &TypePath) -> TypePath {
    let mut abi = interface.clone();
    let last_ident = &mut abi
        .path
        .segments
        .last_mut()
        .expect("closure.path.segments.last_mut()")
        .ident;
    *last_ident = format_ident!("{}_abi", last_ident);
    
    abi
}