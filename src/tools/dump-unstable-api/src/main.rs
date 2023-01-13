//! Dump the unstable API for a feature

use std::{
    collections::HashMap,
    fs,
    io::BufReader,
    path::{Path, PathBuf},
};

use rustdoc_types::{Crate, Id, Item};
use syn::{parse::Parser, Ident, Lit, Meta, NestedMeta};

fn is_ident(ident: &Ident, name: &str) -> bool {
    *ident == Ident::new(name, ident.span())
}

/// Returns a `feature_name` -> Vec<`rustdoc_id`> items mapping.
pub fn load_rustdoc_json_metadata(doc_dir: &Path) -> (Vec<Crate>, HashMap<String, Vec<Id>>) {
    let mut all_items = HashMap::new();
    let mut all_crates = vec![];

    for file in fs::read_dir(doc_dir).expect("failed to list files in directory") {
        let entry = file.expect("failed to list file in directory");
        let file = fs::File::open(entry.path()).expect("failed to open file");
        let krate: Crate =
            serde_json::from_reader(BufReader::new(file)).expect("failed to parse JSON docs");

        let mut crate_items = HashMap::new();
        for (id, item) in &krate.index {
            if item.name.is_none() {
                continue;
            }
            let unstable_feature = item.attrs.iter().find_map(|attr: &String| {
                let Ok(parsed) = syn::Attribute::parse_outer.parse_str(attr).map(|mut v| v.swap_remove(0)) else {return None};

                // Make sure this is an `unstable` attribute.
                if !is_ident(parsed.path.get_ident()?, "unstable") {
                    return None;
                }

                // Given `#[unstable(feature = "xyz")]`, return `(feature = "xyz")`.
                let list = match parsed.parse_meta() {
                    Ok(Meta::List(list)) => list,
                    _ => return None,
                };

                // Given a `NestedMeta` like `feature = "xyz"`, returns `xyz`.
                let get_feature_name = |nested: &_| {
                    match nested {
                        NestedMeta::Meta(Meta::NameValue(name_value)) => {
                            if !is_ident(name_value.path.get_ident()?, "feature") {
                                return None;
                            }
                            match &name_value.lit {
                                Lit::Str(s) => Some(s.value()),
                                _ => None,
                            }
                        }
                        _ => None,
                    }
                };

                for nested in list.nested.iter() {
                    if let Some(feat) = get_feature_name(nested) {
                        return Some(feat);
                    }
                }

                None
            });
            if let Some(feat) = unstable_feature {
                crate_items.insert(id, feat);
            }
        }

        for (id, feat) in crate_items {
            all_items.insert(id.clone(), feat);
        }

        all_crates.push(krate);
    }

    let mut out: HashMap<_, Vec<_>> = HashMap::new();
    for (id, feature) in all_items {
        out.entry(feature).or_default().push(id);
    }

    (all_crates, out)
}

fn extract_item_tree_for_id(crates: &[Crate], id: &Id) -> Vec<Vec<Id>> {
    let path = crates.iter().flat_map(|c| c.paths.get(id)).collect::<Vec<_>>();
    assert_eq!(path.len(), 1);
    let mut built = Vec::new();
    path[0]
        .path
        .iter()
        .map(|seg| {
            built.push(seg.clone());
            built.clone()
        })
        .map(|path_segment| {
            crates
                .iter()
                .flat_map(|c| {
                    c.paths
                        .iter()
                        .find(|&(_, item)| &item.path == &path_segment)
                        .map(|v| v.0.clone())
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn get_item_for_id(crates: &[Crate], id: &Id) -> Option<Item> {
    for c in crates {
        if let Some(item) = c.index.get(id) {
            return Some(item.clone());
        }
    }
    None
}

fn _get_item_name_for_id(crates: &[Crate], id: &Id) -> Vec<String> {
    crates
        .into_iter()
        .flat_map(|c| c.paths.get(id).map(|s| s.path.clone()))
        .next()
        .unwrap_or_default()
}

fn main() {
    let json_docs_path = PathBuf::from(std::env::args_os().nth(1).expect("Need path to json docs"));
    let (crates, mapping) = load_rustdoc_json_metadata(&json_docs_path);
    let items_from_feature = mapping.get("default_free_fn").cloned().unwrap_or(Vec::new());
    let items = extract_item_tree_for_id(&crates, items_from_feature.first().unwrap())
        .into_iter()
        .map(|id| id.into_iter().flat_map(|id| get_item_for_id(&crates, &id)).next().unwrap()).collect::<Vec<_>>();
    println!("{:#?}", items);
}
