use std::collections::{HashMap, HashSet};

// This funciton takes a Rust module path like
// `<T as failure::as_fail::AsFail>::as_fail and strips`
// down the fully-qualified trait paths within to just the base trait name, like
// `<T as AsFail>::as_fail`
fn get_base_trait_name(after_as: &str) -> Option<String> {
    //Read until the first ">" character, which marks the end of the trait path. We do not modify *rest
    let mut parts = after_as.split('>');
    let path = parts.next()?;
    let mut rest: Vec<&str> = parts.collect();
    // This is the "AsFail" in the example
    let basename: &str = *path.split("::").collect::<Vec<&str>>().last()?;
    rest.insert(0, basename);
    Some(rest.join(">"))
}

pub fn simplify_trait_paths(path: &str) -> String {
    let parts: Vec<&str> = path.split(" as ").collect();
    if parts.len() == 1 {
        path.to_string()
    } else {
        parts.into_iter()
            .enumerate()
            .map(|(i, after_as)|
                //Every other segment here is what comes before the " as ", which we do not modify.
                //So just append it to the list and move on
                if i % 2 == 0 {after_as.to_string()} else { get_base_trait_name(after_as).unwrap() }
            )
            .collect::<Vec<String>>()
            // Surgery complete. Stitch it all back up.
            .join(" as ")
    }
}

#[cfg(test)]
mod tests {
    use crate::callgraph_matching::simplify_trait_paths;

    #[test]
    fn test_1() {
        assert_eq!(simplify_trait_paths("<&mut std::collections::hash::table::RawTable<K,V> as std::collections::hash::table::Put<K,V>>::borrow_table_mut".to_string()), "<&mut std::collections::hash::table::RawTable<K,V> as Put<K,V>>::borrow_table_mut");
    }
    #[test]
    fn test_2() {
        assert_eq!(
            simplify_trait_paths(
                "<futures::lock::TryLock<T> as core::ops::deref::Deref>::deref".to_string()
            ),
            "<futures::lock::TryLock<T> as Deref>::deref"
        );
    }
    #[test]
    fn test_3() {
        assert_eq!(simplify_trait_paths("<network::proto::state_synchronizer::RequestChunk as ::protobuf::Message>::default_instance".to_string()), "<network::proto::state_synchronizer::RequestChunk as Message>::default_instance");
    }
    #[test]
    fn test_4() {
        assert_eq!(
            simplify_trait_paths("<T as failure::as_fail::AsFail>::as_fail".to_string()),
            "<T as AsFail>::as_fail"
        );
    }
}

pub struct CallGraph {
    pub label_to_caller_labels: HashMap<String, HashSet<String>>,
    pub short_label_to_labels: HashMap<String, HashSet<String>>,
    pub label_to_short_label: HashMap<String, String>,
}
