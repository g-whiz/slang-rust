use proc_macro2::TokenStream;

/// The helper macros of `slango` whose input the generated code gives as
/// items.
const HELPERS: [&str; 2] = ["wgpu_only", "rhi_only"];

/// Formats generated tokens as a Rust file, with the items inside each call
/// of a helper macro of `slango` formatted as items too, indented inside the
/// call.
pub(crate) fn unparse(tokens: TokenStream) -> String {
    let file = syn::parse2::<syn::File>(tokens).expect("the bindings parse as a Rust file");
    let mut text = String::new();
    let mut plain = Vec::new();
    for item in file.items {
        let helper = match &item {
            syn::Item::Macro(call) => call
                .mac
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
                .filter(|name| HELPERS.contains(&name.as_str())),
            _ => None,
        };
        let (Some(name), syn::Item::Macro(call)) = (helper, &item) else {
            plain.push(item);
            continue;
        };
        text.push_str(&items(std::mem::take(&mut plain)));
        let inner = syn::parse2::<syn::File>(call.mac.tokens.clone())
            .expect("the input of a helper macro parses as items");
        let body: String = items(inner.items)
            .lines()
            .map(|line| {
                if line.is_empty() {
                    "\n".to_owned()
                } else {
                    format!("    {line}\n")
                }
            })
            .collect();
        for part in ["::slango::__private::", &name, "! {\n", &body, "}\n"] {
            text.push_str(part);
        }
    }
    text.push_str(&items(plain));
    text
}

fn items(items: Vec<syn::Item>) -> String {
    if items.is_empty() {
        return String::new();
    }
    prettyplease::unparse(&syn::File {
        shebang: None,
        attrs: Vec::new(),
        items,
    })
}
