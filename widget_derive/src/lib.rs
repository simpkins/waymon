use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/*
// Derive the WaymonWidgetConfig trait for the WidgetConfig enum
#[proc_macro_derive(WaymonWidgetConfigEnum)]
pub fn derive_waymon_widget_config_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    if let syn::Data::Enum(ref _data) = input.data {
        let name = input.ident;

        return TokenStream::from(quote!(
            impl for WaymonWidgetConfigEnum #name {
                fn create_widget(&self) {
                    eprintln!("create widget running");
                }
            }
        ));
    }

    TokenStream::from(
        syn::Error::new(
            input.ident.span(),
            "Can only derive WaymonWidget on an enum of widget configs",
        )
        .to_compile_error(),
    )
}
*/

/// Derive the WaymonWidgetConfig trait for a widget config struct
#[proc_macro_derive(WaymonWidgetConfig)]
pub fn derive_waymon_widget_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match input.data {
        syn::Data::Struct(ref _data) => derive_widget_config_struct(input),
        syn::Data::Enum(ref data_enum) => derive_widget_config_enum(input.ident, data_enum),
        _ => TokenStream::from(
            syn::Error::new(
                input.ident.span(),
                "Can only derive WaymonWidget on an enum of widget configs",
            )
            .to_compile_error(),
        ),
    }
}

fn derive_widget_config_enum(name: syn::Ident, data_enum: &syn::DataEnum) -> TokenStream {
    // Emit the match cases for all of the enum variants
    let mut body = quote!();
    for v in &data_enum.variants {
        let vname = &v.ident;
        body = quote!(
            #body
            #name::#vname(cfg) => cfg.create_widget(all_stats, history_length),
        );
    }

    TokenStream::from(quote!(
    impl WaymonWidgetConfig for #name {
        fn create_widget(
            &self,
            all_stats: &mut crate::stats::AllStats,
            history_length: usize,
        ) -> std::rc::Rc<std::cell::RefCell<dyn crate::widgets::Widget>> {
            match self {
                #body
            }
        }
    }
    ))
}

fn derive_widget_config_struct(input: DeriveInput) -> TokenStream {
    let name = input.ident;
    let name_str = name.to_string();
    let widget_name = match name_str.strip_suffix("Config") {
        Some(widget_name_str) => Ident::new(widget_name_str, Span::call_site()),
        None => {
            return TokenStream::from(
                syn::Error::new(
                    name.span(),
                    "widget config name {:?} does not end in \"Config\": \
                        cannot automatically determine widget struct name",
                )
                .to_compile_error(),
            );
        }
    };

    TokenStream::from(quote!(
    impl WaymonWidgetConfig for #name {
        fn create_widget(
            &self,
            all_stats: &mut crate::stats::AllStats,
            history_length: usize,
        ) -> std::rc::Rc<std::cell::RefCell<dyn crate::widgets::Widget>> {
            #widget_name::new(self, all_stats, history_length)
        }
    }
    ))
}
