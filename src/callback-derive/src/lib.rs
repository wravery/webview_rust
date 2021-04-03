extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{AttributeArgs, ItemStruct, Lit, Meta, parse_macro_input};

#[proc_macro_attribute]
pub fn completed_callback(attr: TokenStream, input: TokenStream) -> TokenStream {
    let attr = parse_macro_input!(attr as AttributeArgs);
    let ast = parse_macro_input!(input as ItemStruct);
    impl_completed_callback(&attr, &ast)
}

fn impl_completed_callback(attr: &AttributeArgs, ast: &ItemStruct) -> TokenStream {
    let name = &ast.ident;

    
    let mut interface = None;
    let mut arg_1 = None;
    let mut arg_2 = None;
    let name_value_attrs = ast.attrs.iter().filter_map(|attr| match attr.parse_meta() {
        Ok(Meta::NameValue(value)) => Some((value.path, value.lit)),
        _ => None,
    });
    for (path, lit) in name_value_attrs {
        match (path.get_ident(), lit) {
            (Some(ident), Lit::Str(value)) => match ident.to_string().as_str() {
                "interface" => {
                    interface = Some(value.value());
                }
                "arg_1" => {
                    arg_1 = Some(value.value());
                }
                "arg_2" => {
                    arg_2 = Some(value.value());
                }
                _ => (),
            },
            _ => (),
        }
    }

    let gen = match (interface, arg_1, arg_2) {
        (Some(interface), Some(arg_1), Some(arg_2)) => {
            let closure = format!("{}Closure", name.to_string());
            let abi = format!("{}_abi", interface);

            quote! {
                type #closure = CompletedClosure<#arg_1::Output, #arg_2::Output>;

                #[automatically_derived]
                impl #name {
                    pub fn new(completed: #closure) -> Self {
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

                #[automatically_derived]
                impl CallbackInterface<#interface> for #name {
                    fn refcount(&self) -> &AtomicU32 {
                        &self.refcount
                    }
                }

                #[automatically_derived]
                impl CompletedCallback<#interface, #arg_1, #arg_2> for #name {
                    fn completed(&mut self) -> Option<#closure> {
                        self.completed.take()
                    }
                }
            }
        }
        _ => quote! {},
    };

    gen.into()
}
